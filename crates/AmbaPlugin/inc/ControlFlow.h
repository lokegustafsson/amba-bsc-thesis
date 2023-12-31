#pragma once

#include <memory>
#include <s2e/S2EExecutionState.h>

#include <unordered_map>
#include <vector>

#include "Numbers.h"
#include "Amba.h"
#include "LibambaRs.h"
#include "HashableWrapper.h"

namespace control_flow {

namespace types {

using StateIdS2E = hashable_wrapper::HashableWrapper<i32, 0>;
using StateIdAmba = hashable_wrapper::HashableWrapper<u64, 1>;
using StatePC = hashable_wrapper::HashableWrapper<u64, 2>;
using BasicBlockGeneration = hashable_wrapper::HashableWrapper<u8, 3>;
using VarValuePair = std::pair<std::string, std::vector<unsigned char>>;
using ConcreteInputs = std::vector<VarValuePair>;

struct StateMetadata {
	StateIdAmba amba_state_id;
	StateIdS2E s2e_state_id;
	ConcreteInputs concrete_inputs;

	NodeMetadataFFI into_ffi()  const;
};

struct BasicBlockMetadata {
	StateIdAmba symbolic_state_id;
	u64 basic_block_vaddr;
	u64 basic_block_generation;
	u64 basic_block_elf_vaddr;
	std::vector<u8> basic_block_content;

	NodeMetadataFFI into_ffi() const;
};

}

using namespace types;

StateIdS2E getStateIdS2E(s2e::S2EExecutionState *);

ConcreteInputsFFI concreteInputsIntoFFI(ConcreteInputs);

class ControlFlow {
  public:
	ControlFlow(std::string);

	const char *getName() const;
	u64 states() const;
	std::vector<NodeMetadataFFIPair> &edges();

  protected:
	StateIdAmba getStateIdAmba(StateIdS2E);
	void incrementStateIdAmba(StateIdS2E);

	const std::string m_name;

	u64 state_count = 0;
	std::unordered_map<StateIdS2E, StateIdAmba> m_states {};
	std::vector<NodeMetadataFFIPair> m_new_edges {};
};

} // namespace control_flow
