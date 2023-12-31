CXX ?= g++
LIBS := \
	-idirafter $(S2E_PATH)/klee/include \
	-idirafter $(S2E_PATH)/libcoroutine/include \
	-idirafter $(S2E_PATH)/libcpu/include \
	-idirafter $(S2E_PATH)/libfsigc++/include \
	-idirafter $(S2E_PATH)/libq/include \
	-idirafter $(S2E_PATH)/libs2e/include \
	-idirafter $(S2E_PATH)/libs2ecore/include \
	-idirafter $(S2E_PATH)/libs2eplugins/src \
	-idirafter $(S2E_PATH)/libtcg/include \
	-idirafter $(S2E_PATH)/libvmi/include \
	-idirafter $(S2E_PATH)/tools/lib/X8664BitcodeLibrary \
	-idirafter $(LUA_PATH) \
	-idirafter $(BOOST_PATH) \
	-idirafter $(GLIBC_PATH) \
	-idirafter $(LLVM_PATH) \
	-idirafter $(ZYDIS_PATH)/include \
	-idirafter $(ZYCORE_PATH)/include
ARCHIVES := \
	-L $(ZYDIS_PATH)/lib \
	-l Zydis \
	-L $(ZYCORE_PATH)/lib \
	-l Zycore \
	-L . \
	-l amba
OPT-LEVEL ?= -g3 -O0
CXX-FLAGS := \
	-std=c++20 \
	-Wall \
	-Wextra \
	-Wno-unused-parameter \
	-Werror=missing-field-initializers \
	$(OPT-LEVEL) \
	-U _FORTIFY_SOURCE \
	-I inc \
	$(LIBS) \
	-DBOOST_BIND_GLOBAL_PLACEHOLDERS=1 \
	-DTARGET_PAGE_BITS=12 \
	-DSE_RAM_OBJECT_BITS=12 \
	-D"SE_RAM_OBJECT_MASK=(~11)"
LDFLAGS = -Wl,-soname,$@
S2E_PATH ?= ../../s2e

libamba.so: \
  Amba.o Zydis.o StatePrioritisation.o HeapLeak.o ControlFlow.o \
  SymbolicGraph.o AssemblyGraph.o AmbaPlugin.o \
  libamba.a
	$(CXX) -shared $^ $(ARCHIVES) $(LDFLAGS) -o $@

libamba.a:
	if [ -v IMPURE_RUST ]; then \
		make libamba_impure; \
	else \
		cp $(LIBAMBA_PATH)/libamba.a . ; \
	fi

libamba_impure: ../libamba/*
	cargo build -p libamba
	cp ../../target/debug/libamba.a .
	touch libamba_impure

Amba.o: \
  src/Amba.cpp \
  inc/Amba.h inc/Numbers.h
	$(CXX) src/Amba.cpp $(CXX-FLAGS) -c -o $@

AmbaPlugin.o: \
  src/AmbaPlugin.cpp \
  inc/AmbaPlugin.h inc/Numbers.h
	$(CXX) src/AmbaPlugin.cpp $(CXX-FLAGS) -c -o $@

StatePrioritisation.o: \
  src/StatePrioritisation.cpp \
  inc/StatePrioritisation.h inc/AmbaException.h inc/Numbers.h
	$(CXX) src/StatePrioritisation.cpp $(CXX-FLAGS) -c -o $@

HeapLeak.o: \
  src/HeapLeak.cpp \
  inc/HeapLeak.h inc/AmbaException.h inc/Numbers.h
	$(CXX) src/HeapLeak.cpp $(CXX-FLAGS) -c -o $@

ControlFlow.o: \
  src/ControlFlow.cpp \
  inc/ControlFlow.h inc/AmbaException.h inc/Numbers.h
	$(CXX) src/ControlFlow.cpp $(CXX-FLAGS) -c -o $@

SymbolicGraph.o: \
  src/SymbolicGraph.cpp \
  inc/SymbolicGraph.h inc/AmbaException.h inc/Numbers.h
	$(CXX) src/SymbolicGraph.cpp $(CXX-FLAGS) -c -o $@

AssemblyGraph.o: \
  src/AssemblyGraph.cpp \
  inc/AssemblyGraph.h inc/AmbaException.h inc/Numbers.h
	$(CXX) src/AssemblyGraph.cpp $(CXX-FLAGS) -c -o $@

Zydis.o: \
  src/Zydis.cpp \
  inc/Zydis.h inc/AmbaException.h inc/Numbers.h
	$(CXX) src/Zydis.cpp $(CXX-FLAGS) -c -o $@

Test: \
  src/Test.cpp \
  inc/Numbers.h inc/AmbaException.h \
  Zydis.o Amba.o \
  libamba.a
	$(CXX) src/Test.cpp Zydis.o $(CXX-FLAGS) $(ARCHIVES) -o $@

TAGS: inc/* src/*
	ctags -R -e

tags: inc/* src/*
	ctags -R

../../compile_flags.txt: compile_flags_template.txt
	cat $^ \
	| sed "s|LLVM_PATH|${LLVM_PATH}|" \
	| sed "s|BOOST_PATH|${BOOST_PATH}|" \
	| sed "s|GLIBC_PATH|${GLIBC_PATH}|" \
	| sed "s|ZYDIS_PATH|${ZYDIS_PATH}/include|" \
	| sed "s|ZYCORE_PATH|${ZYCORE_PATH}/include|" \
	| sed "s|LUA_PATH|$(LUA_PATH)|" \
	> $@

compile_flags.txt: ../../compile_flags.txt

test:
	cd ../.. && \
	time impure-amba run demos/hello.recipe.json --no-gui

clean:
	-rm *.o
	-rm *.a
	-rm *.so
	-rm *.out
	-rm Test
	-rm ../../compile_flags.txt
	-rm compile_flags.txt
	-rm libamba_impure
	-cargo clean

.PHONY: clean libamba.a test
