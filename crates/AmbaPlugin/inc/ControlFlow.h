#pragma once

#include <s2e/S2EExecutionState.h>

#include <unordered_map>

#include "Numbers.h"
#include "Amba.h"
#include "LibambaRs.h"
#include "HashableWrapper.h"

namespace control_flow {

// Values used as keys need to have I = 0 or else the default
// constructor is implicitly deleted?
using UidS2E = hashable_wrapper::HashableWrapper<i32, 0>;
using StatePC = hashable_wrapper::HashableWrapper<u64, 0>;

using AmbaId = hashable_wrapper::HashableWrapper<u64, 1>;
using Generation = hashable_wrapper::HashableWrapper<u8, 2>;
using Packed = hashable_wrapper::HashableWrapper<u64, 3>;

void updateControlFlowGraph(ControlFlowGraph *, Packed, Packed);

class ControlFlow {
  public:
	ControlFlow(std::string);
	~ControlFlow();

	amba::TranslationFunction translateBlockStart;
	amba::ExecutionFunction onBlockStart;
	amba::SymbolicExecutionFunction onStateFork;
	amba::StateMergeFunction onStateMerge;

	const char *getName() const;
	ControlFlowGraph *cfg();

  protected:
	StatePC toAlias(UidS2E, u64);
	Packed getBlockId(s2e::S2EExecutionState *, u64);

	const std::string m_name;
	ControlFlowGraph *const m_cfg;

	/// State uuid → reuses
	std::unordered_map<UidS2E, Packed> m_uuids {};

	/// (State, pc) → gen
	std::unordered_map<StatePC, Generation> m_generations {};

	/// Either:
	/// State → (State, pc)
	/// Alias → Alias
	std::unordered_map<u64, u64> m_last {};
};

} // namespace control_flow
