#include "ControlFlow.h"
#include "AmbaException.h"

namespace control_flow {

void updateControlFlowGraph(ControlFlowGraph *cfg, Metadata from, Metadata to) {
	const auto from_ = from.into_ffi();
	const auto to_ = to.into_ffi();
	rust_update_control_flow_graph(
		cfg,
		from_,
		to_
	);
}

NodeMetadataFFI Metadata::into_ffi() const {
	return (NodeMetadataFFI) {
		.symbolic_state_id = (u32) this->symbolic_state_id.val,
		.basic_block_vaddr = this->basic_block_vaddr,
		.basic_block_generation = this->basic_block_generation
	};
}

StateIdS2E getStateIdS2E(s2e::S2EExecutionState *state) {
	return StateIdS2E(state->getGuid());
}

ControlFlow::ControlFlow(std::string name)
	: m_name(name)
	, m_cfg(rust_new_control_flow_graph())
{}

ControlFlow::~ControlFlow() {
	rust_print_graph_size(this->m_name.c_str(), this->m_cfg);
	rust_free_control_flow_graph(this->m_cfg);
}

const char *ControlFlow::getName() const {
	return this->m_name.c_str();
}

ControlFlowGraph *ControlFlow::cfg() {
	return this->m_cfg;
}

u64 ControlFlow::states() const {
	return this->state_count;
}

StateIdAmba ControlFlow::getStateIdAmba(StateIdS2E id) {
	auto& amba_id = this->m_states[id];
	if (amba_id == 0) {
		this->state_count++;
		amba_id.val = this->state_count;
	}
	return amba_id;
}

void ControlFlow::incrementStateIdAmba(StateIdS2E id) {
	this->state_count++;
	auto& amba_id = this->m_states[id];
	amba_id.val = this->state_count;
}

} // namespace control_flow
