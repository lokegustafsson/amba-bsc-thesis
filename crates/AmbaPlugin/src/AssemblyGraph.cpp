#include <s2e/Plugins/OSMonitors/ModuleDescriptor.h>
#include <s2e/Plugins/OSMonitors/Support/ModuleMap.h>
#include <s2e/Utils.h>
#include <tcg/tb.h>

#include <vector>

#include "AssemblyGraph.h"
#include "AmbaException.h"
#include "ControlFlow.h"

namespace assembly_graph {

AssemblyGraph::AssemblyGraph(std::string name, s2e::plugins::ModuleMap *module_map)
	: ControlFlow(name)
	, m_module_map(module_map)
{}

void AssemblyGraph::translateBlockComplete(
	s2e::S2EExecutionState *state,
	TranslationBlock *tb,
	u64 final_instruction_pc
) {
	const u64 tb_vaddr = tb->pc;
	const u64 tb_len = tb->size;

	u64 elf_vaddr = 0;
	s2e::ModuleDescriptorConstPtr mod = this->m_module_map->getModule(state);
	if (mod != nullptr) {
		bool ok = mod->ToNativeBase(tb_vaddr, elf_vaddr);
		assert(ok);
	}

	std::vector<u8> cached_tb_content(tb_len);
	// BEWARE: Extracting information from the TranslationBlock and other S2E
	// callback data has a lot of subtle complexity. Tread carefully. If this memory
	// read leads to "KLEE: WARNING: silently concretizing", you have a bug.
	bool ok = state->mem()->read(tb_vaddr, cached_tb_content.data(), tb_len);
	if (!ok) {
		*amba::warning_stream()
			<< "Failed TB read: pc=" << s2e::hexval(tb->pc)
			<< " cs_base=" << s2e::hexval(tb->cs_base)
			<< " flags=" << s2e::hexval(tb->flags)
			<< " size=" << s2e::hexval(tb->size)
			<< " cflags=" << s2e::hexval(tb->cflags)
			<< " tc.ptr=" << s2e::hexval(tb->tc.ptr)
			<< " tc.size=" << s2e::hexval(tb->tc.size)
			<< "\n";
	}

	const StatePC key = this->packStatePc(
		control_flow::getStateIdS2E(state),
		tb_vaddr
	);
	++this->m_translation_block_metadata[key].generation.val;
	this->m_translation_block_metadata[key].elf_vaddr = elf_vaddr;
	this->m_translation_block_metadata[key].content = cached_tb_content;
}

void AssemblyGraph::onBlockStart(
	s2e::S2EExecutionState *state,
	u64 pc
) {
	const StateIdAmba amba_id = this->getStateIdAmba(control_flow::getStateIdS2E(state));
	const BasicBlockMetadata curr = this->getMetadata(state, pc);
	// Will insert 0 if value doesn't yet exist
	BasicBlockMetadata &last = this->m_last_executed_bb[amba_id];
	this->m_new_edges.push_back(
		(NodeMetadataFFIPair) {
			.fst = last.into_ffi(),
			.snd = curr.into_ffi()
		}
	);
	last = curr;
}

void AssemblyGraph::onStateFork(
	s2e::S2EExecutionState *old_state,
	const std::vector<s2e::S2EExecutionState *> &new_states,
	const std::vector<klee::ref<klee::Expr>> &conditions
) {
	const StateIdAmba old_amba_id = this->getStateIdAmba(control_flow::getStateIdS2E(old_state));
	const BasicBlockMetadata old_last = this->m_last_executed_bb[old_amba_id];

	this->incrementStateIdAmba(control_flow::getStateIdS2E(old_state));

	for (auto& new_state : new_states) {
		const StateIdAmba new_amba_id = this->getStateIdAmba(control_flow::getStateIdS2E(new_state));
		this->m_last_executed_bb[new_amba_id] = old_last;
	}
}

void AssemblyGraph::onStateMerge(
	s2e::S2EExecutionState *destination_state,
	s2e::S2EExecutionState *source_state
) {
	this->incrementStateIdAmba(control_flow::getStateIdS2E(destination_state));
}

StatePC AssemblyGraph::packStatePc(StateIdS2E uid, u64 pc) {
	return pc << 4 | (u64) uid.val;
}

BasicBlockMetadata AssemblyGraph::getMetadata(
	s2e::S2EExecutionState *s2e_state,
	u64 pc
) {
	const StateIdS2E state = StateIdS2E(s2e_state->getID());
	const StateIdAmba amba_id = this->getStateIdAmba(state);
	const StatePC state_pc = this->packStatePc(state, pc);
	const TranslationBlockMetadata tb_meta = this->m_translation_block_metadata[state_pc];

	return (BasicBlockMetadata) {
		.symbolic_state_id = amba_id,
		.basic_block_vaddr = pc,
		.basic_block_generation = tb_meta.generation.val,
		.basic_block_elf_vaddr = tb_meta.elf_vaddr,
		.basic_block_content = tb_meta.content,
	};
}

}
