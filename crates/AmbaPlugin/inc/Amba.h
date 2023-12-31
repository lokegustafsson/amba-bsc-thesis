#pragma once

#include <cpu/types.h>
#include <s2e/S2EExecutionState.h>
#include <llvm/Support/raw_ostream.h>

#include <compare>
#include <optional>
#include <functional>

#include "Numbers.h"
#include "Zydis.h"

namespace s2e {
class ModuleDescriptor;
}

namespace amba {

extern std::function<llvm::raw_ostream *()> debug_stream;
extern std::function<llvm::raw_ostream *()> info_stream;
extern std::function<llvm::raw_ostream *()> warning_stream;

using TranslationFunction = void (
	s2e::ExecutionSignal *,
	s2e::S2EExecutionState *,
	TranslationBlock *,
	u64
);
using TranslationCompleteFunction = void (
	s2e::S2EExecutionState *,
	TranslationBlock *,
	u64
);
using ExecutionFunction = void (s2e::S2EExecutionState *, u64);
using SymbolicExecutionFunction = void (
	s2e::S2EExecutionState *,
	const std::vector<s2e::S2EExecutionState *> &,
	const std::vector<klee::ref<klee::Expr>> &
);
using StateMergeFunction = void (
	s2e::S2EExecutionState *,
	s2e::S2EExecutionState *
);
using StateKillFunction = void (s2e::S2EExecutionState *);
using TimerFunction = void ();
using ModuleFunction = void (s2e::S2EExecutionState *, const s2e::ModuleDescriptor &);
using ProcessFunction = void (s2e::S2EExecutionState *, const u64, const u64, const u64);

struct AddressLengthPair {
	target_phys_addr_t adr;
	target_phys_addr_t size;

	auto operator<=>(const AddressLengthPair &rhs) const = default;
	bool operator==(const AddressLengthPair &rhs) const = default;
};

constexpr size_t MAX_INSTRUCTION_LENGTH = 15; // bytes

std::optional<target_phys_addr_t> readOperandAddress(
	const CPUX86State &cpu_state,
	const ZydisDecodedOperand operand
);

std::array<uint8_t, MAX_INSTRUCTION_LENGTH> readConstantMemory(S2EExecutionState *state, uint64_t pc);

zydis::Instruction readInstruction(s2e::S2EExecutionState *state, u64 pc);

bool isStackAddress(const CPUX86State &state, target_phys_addr_t adr);

u64 readRegister(const CPUX86State &state, const ZydisRegister reg);

} // namespace amba
