#include "ControlFlow.h"
#include "AmbaException.h"

namespace control_flow {

u64 ControlFlow::getBlockId(
	s2e::S2EExecutionState *state,
	u64 pc
) {
	// 128 bit integers don't have a hash function and can't be used in std::unordered_map.
	// Neither do random structs or tuples.
	// Functions have a 16 bit alignment requirement anyway, so for now, let's hope that
	// self modifying code won't touch the same address more than 16 times.
	const u64 id = this->m_uuids[state->getID()] & 15;
	return id | pc;
}

ControlFlow::ControlFlow(std::string name)
	: m_name(name)
	, m_cfg(rust_new_control_flow_graph())
	{}

ControlFlow::~ControlFlow() {
	rust_print_graph_size(this->m_name.c_str(), this->m_cfg);
	rust_free_control_flow_graph(this->m_cfg);
}

void ControlFlow::onBlockStart(
	s2e::S2EExecutionState *s2e_state,
	u64 pc
) {
	// Will insert 0 if value doesn't yet exist
	auto &last = this->m_last[s2e_state->getID()];
	rust_update_control_flow_graph(
		this->m_cfg,
		last,
		pc
	);
	last = pc;
}

void ControlFlow::onStateFork(
	s2e::S2EExecutionState *old_state,
	const std::vector<s2e::S2EExecutionState *> &new_states,
	const std::vector<klee::ref<klee::Expr>> &conditions
) {
	const auto old_id = old_state->getID();
	const auto from = this->m_uuids[old_id];
	const auto last_raw = this->m_last[old_id];

	for (auto &new_state : new_states) {
		const auto new_id = new_state->getID();

		AMBA_ASSERT(new_id != old_id);

		this->m_uuids[new_id] = ++this->m_last_uuid;
		this->m_last[new_id] = last_raw;

		rust_update_control_flow_graph(
			this->m_cfg,
			from,
			this->m_last_uuid
		);
	}
}

void ControlFlow::onStateMerge(
	s2e::S2EExecutionState *destination_state,
	s2e::S2EExecutionState *source_state
) {
	const auto dest_id = destination_state->getID();
	const auto src_id = source_state->getID();

	const auto from_left = this->m_uuids[(i32) dest_id];
	const auto from_right = this->m_uuids[(i32) src_id];

	this->m_uuids[(i32) dest_id] = ++this->m_last_uuid;

	rust_update_control_flow_graph(
		this->m_cfg,
		from_left,
		this->m_last_uuid
	);
	rust_update_control_flow_graph(
		this->m_cfg,
		from_right,
		this->m_last_uuid
	);
}

void ControlFlow::onTimer() {
	rust_ipc_send_graph(this->m_name.c_str(), this->m_cfg);
}

void ControlFlow::onEngineShutdown() {
	rust_ipc_send_graph(this->m_name.c_str(), this->m_cfg);
}

} // namespace control_flow
