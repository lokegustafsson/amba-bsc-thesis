-- This section configures the S2E engine.
s2e = {
    logging = {
        -- Possible values include "all", "debug", "info", "warn" and "none".
        -- See Logging.h in libs2ecore.
        console = "debug",
        logLevel = "debug",
    },

    -- All the cl::opt options defined in the engine can be tweaked here.
    -- This can be left empty most of the time.
    -- Most of the options can be found in S2EExecutor.cpp and Executor.cpp.
    kleeArgs = {
    },
}

-- Declare empty plugin settings. They will be populated in the rest of
-- the configuration file.
plugins = {}
pluginsConfig = {}

-- Include various convenient functions
dofile('library.lua')

-------------------------------------------------------------------------------
-- This plugin contains the core custom instructions.
-- Some of these include s2e_make_symbolic, s2e_kill_state, etc.
-- You always want to have this plugin included.

add_plugin("BaseInstructions")
pluginsConfig.BaseInstructions = {
    {% if enable_tickler %}
    -- When doing malware analysis, we do not want malware to be able to (easily) run S2E commands.
    -- We therefore use this option to restrict access to these instructions to the first
    -- process that called an S2E instruction. Kernel is not restricted.
    restrict = true
    {% endif %}
}

-------------------------------------------------------------------------------
-- This plugin implements "shared folders" between the host and the guest.
-- Use it in conjunction with s2eget and s2eput guest tools in order to
-- transfer files between the guest and the host.

add_plugin("HostFiles")
pluginsConfig.HostFiles = {
    baseDirs = {
        "{{ project_dir }}",
        {% if use_seeds == true %}
        "{{ seeds_dir }}",
        {% endif %}
    },
    allowWrite = true,
}

-------------------------------------------------------------------------------
-- This plugin provides support for virtual machine introspection and binary
-- formats parsing. S2E plugins can use it when they need to extract
-- information from binary files that are either loaded in virtual memory
-- or stored on the host's file system.

add_plugin("Vmi")
pluginsConfig.Vmi = {
    baseDirs = {
        "{{ project_dir }}",
        {% if has_guestfs %}
            {% for path in guestfs_paths %}
            "{{ path }}",
            {% endfor %}
        {% endif %}
    },
}

-------------------------------------------------------------------------------
-- This plugin provides various utilities to read from process memory.
-- In case it is not possible to read from guest memory, the plugin tries
-- to read static data from binary files stored in guestfs.
add_plugin("MemUtils")

-------------------------------------------------------------------------------
-- This plugin collects various execution statistics and sends them to a QMP
-- server that listens on an address:port configured by the S2E_QMP_SERVER
-- environment variable.
--
-- The "s2e run {{ project_name }}" command sets up such a server in order to display
-- stats on the dashboard.
--
-- You may also want to use this plugin to integrate S2E into a larger
-- system. The server could collect information about execution from different
-- S2E instances, filter them, and store them in a database.

add_plugin("WebServiceInterface")
pluginsConfig.WebServiceInterface = {
    statsUpdateInterval = 2
}

-------------------------------------------------------------------------------
-- This is the main execution tracing plugin.
-- It generates the ExecutionTracer.dat file in the s2e-last folder.
-- That files contains trace information in a binary format. Other plugins can
-- hook into ExecutionTracer in order to insert custom tracing data.
--
-- This is a core plugin, you most likely always want to have it.

add_plugin("ExecutionTracer")

-------------------------------------------------------------------------------
-- This plugin records events about module loads/unloads and stores them
-- in ExecutionTracer.dat.
-- This is useful in order to map raw program counters and pids to actual
-- module names.

add_plugin("ModuleTracer")

-------------------------------------------------------------------------------
-- This is a generic plugin that let other plugins communicate with each other.
-- It is a simple key-value store.
--
-- The plugin has several modes of operation:
--
-- 1. local: runs an internal store private to each instance (default)
-- 2. distributed: the plugin interfaces with an actual key-value store server.
-- This allows different instances of S2E to communicate with each other.

add_plugin("KeyValueStore")

-------------------------------------------------------------------------------
-- Records statistics into s2e-last/stats.csv.
-- This corresponds to the old klee run.stats file.
add_plugin("StatsTracker")

-------------------------------------------------------------------------------
-- Records the program counter of executed translation blocks.
-- Generates a json coverage file. This file can be later processed by other
-- tools to generate line coverage information. Please refer to the S2E
-- documentation for more details.

add_plugin("TranslationBlockCoverage")
pluginsConfig.TranslationBlockCoverage = {
    writeCoverageOnStateKill = true,
    writeCoverageOnStateSwitch = true,
}

-------------------------------------------------------------------------------
-- Tracks execution of specific modules.
-- Analysis plugins are often interested only in small portions of the system,
-- typically the modules under analysis. This plugin filters out all core
-- events that do not concern the modules under analysis. This simplifies
-- code instrumentation.
-- Instead of listing individual modules, you can also track all modules by
-- setting configureAllModules = true

add_plugin("ModuleExecutionDetector")
pluginsConfig.ModuleExecutionDetector = {
    configureAllModules = true,
    --{% for m in modules %}
    --mod_0 = {
    --    moduleName = "{{ m[0] }}",
    --},
    --{% endfor %}
    logLevel="info"
}

-------------------------------------------------------------------------------
-- This plugin controls the forking behavior of S2E.

add_plugin("ForkLimiter")
pluginsConfig.ForkLimiter = {
    -- How many times each program counter is allowed to fork.
    -- -1 for unlimited.
    maxForkCount = -1,

    -- How many seconds to wait before allowing an S2E process
    -- to spawn a child. When there are many states, S2E may
    -- spawn itself into multiple processes in order to leverage
    -- multiple cores on the host machine. When an S2E process A spawns
    -- a process B, A and B each get half of the states.
    --
    -- In some cases, when states fork and terminate very rapidly,
    -- one can see flash crowds of S2E instances. This decreases
    -- execution efficiency. This parameter forces S2E to wait a few
    -- seconds so that more states can accumulate in an instance
    -- before spawning a process.
    processForkDelay = 5,
}

-------------------------------------------------------------------------------
-- This plugin tracks execution of processes.
-- This is the preferred way of tracking execution and will eventually replace
-- ModuleExecutionDetector.

add_plugin("ProcessExecutionDetector")
pluginsConfig.ProcessExecutionDetector = {
    moduleNames = {
        {% for p in processes %}
        "{{ p }}",
        {% endfor %}
    },
}

-------------------------------------------------------------------------------
-- Keeps for each state/process an updated map of all the loaded modules.
add_plugin("ModuleMap")
pluginsConfig.ModuleMap = {
  logLevel = "info"
}


-------------------------------------------------------------------------------
-- Keeps for each process in ProcessExecutionDetector an updated map
-- of memory regions.
add_plugin("MemoryMap")
pluginsConfig.MemoryMap = {
  logLevel = "info"
}

{% if use_cupa == true %}

-------------------------------------------------------------------------------
-- MultiSearcher is a top-level searcher that allows switching between
-- different sub-searchers.
add_plugin("MultiSearcher")

-- CUPA stands for Class-Uniform Path Analysis. It is a searcher that groups
-- states into classes. Each time the searcher needs to pick a state, it first
-- chooses a class, then picks a state in that class. Classes can further be
-- subdivided into subclasses.
--
-- The advantage of CUPA over other searchers is that it gives similar weights
-- to different parts of the program. If one part forks a lot, a random searcher
-- would most likely pick a state from that hotspot, decreasing the probability
-- of choosing another state that may have better chance of covering new code.
-- CUPA avoids this problem by grouping similar states together.

add_plugin("CUPASearcher")
pluginsConfig.CUPASearcher = {
    -- The order of classes is important, please refer to the plugin
    -- source code and documentation for details on how CUPA works.
    classes = {
        {% if use_seeds == true %}
        -- This is a special class that must be used first when the SeedSearcher
        -- is enabled. It ensures that seed state 0 is kept out of scheduling
        -- unless instructed by SeedSearcher.
        "seed",
        {% endif %}

        -- This ensures that states run for a certain amount of time.
        -- Otherwise too frequent state switching may decrease performance.
        "batch",

        {% if enable_pov_generation %}
        -- This class is used with the Recipe plugin in order to prioritize
        -- states that have a high chance of containing a vulnerability.
        "group",
        {% endif %}

        -- A program under test may be composed of several binaries.
        -- We want to give equal chance to all binaries, even if some of them
        -- fork a lot more than others.
        "pagedir",

        -- Finally, group states by program counter at fork.
        "pc",
    },
    logLevel="info",
    enabled = true,

    -- Delay (in seconds) before switching states (when used with the "batch" class).
    -- A very large delay becomes similar to DFS (current state keeps running
    -- until it is terminated).
    batchTime = 5
}

{% endif %}

{% if use_seeds == true %}

-- Required dependency of SeedSearcher
add_plugin("MultiSearcher")

-------------------------------------------------------------------------------
-- The SeedSearcher plugin looks for new seeds in the seed directory and
-- schedules the seed fetching state whenever a new seed is available. This
-- searcher may be used in conjunction with a fuzzer in order to combine the
-- speed of a fuzzer with the efficiency of symbolic execution. Fuzzers can
-- quickly generate skeleton paths and symbolic execution can explore side
-- branches efficiently along these skeleton paths.
--
-- Note:
--   1. SeedSearcher must be used in conjunction with a suitable bootstrap.sh.
--   Everything is taken care of by s2e-env, just enable the use seeds option.
--
--   2. There will always be an S2E instance running, even if there are
--   otherwise no more states to run. This is because in seeding mode, state 0
--   never terminates, as it continuously tries to fetch new seeds.

add_plugin("SeedSearcher")
pluginsConfig.SeedSearcher = {
    enableSeeds = true,
    seedDirectory = "{{ project_dir }}/seeds",

    -- Save a copy of fetched seeds in s2e-last. This is useful in case
    -- you modify seeds between runs and would like to keep track of the history.
    backupSeeds = true,
}

-- The SeedScheduler plugin takes care of implementing the seed usage policies.
-- It decides when it is a good time to try new seeds, based on current
-- coverage, number of bugs found, etc. When it thinks that S2E is stuck and
-- does not make progress, this plugin will instruct SeedSearcher to schedule
-- a new seed.
add_plugin("SeedScheduler")
pluginsConfig.SeedScheduler = {
    -- Seeds with priority equal to or lower than the threshold are considered
    -- low priority. For CFE, high priorities range from 10 to 7 (various
    -- types of POVs and crashes), while normal test cases are from 6 and
    -- below. High priority seeds are scheduled asap, even if S2E is making
    -- progress.
    lowPrioritySeedThreshold = 6,
}

-- Required for SeedScheduler
add_plugin("TranslationBlockCoverage")
{% endif %}

-------------------------------------------------------------------------------
-- Function models help drastically reduce path explosion. A model is an
-- expression that efficiently encodes the behavior of a function. In imperative
-- languages, functions often have if-then-else branches and loops, which
-- may cause path explosion. A model compresses this into a single large
-- expression. Models are most suitable for side-effect-free functions that
-- fork a lot. Please refer to models.lua and the documentation for more details.

add_plugin("StaticFunctionModels")

pluginsConfig.StaticFunctionModels = {
  modules = {}
}

g_function_models = {}
safe_load('models.lua')
pluginsConfig.StaticFunctionModels.modules = g_function_models

{% if use_test_case_generator %}
-------------------------------------------------------------------------------
-- This generates test cases when a state crashes or terminates.
-- If symbolic inputs consist of symbolic files, the test case generator writes
-- concrete files in the S2E output folder. These files can be used to
-- demonstrate the crash in a program, added to a test suite, etc.

add_plugin("TestCaseGenerator")
pluginsConfig.TestCaseGenerator = {
    generateOnStateKill = true,
    generateOnSegfault = true
}
{% endif %}


{% if enable_pov_generation %}

-------------------------------------------------------------------------------
-- This plugin aggregates different sources of vulnerability information and
-- uses it to generate PoVs.

add_plugin("PovGenerationPolicy")

-------------------------------------------------------------------------------
-- The Recipe plugin continuously monitors execution and looks for states
-- that can be exploited. The most important marker of a vulnerability is
-- dereferencing a symbolic pointer. The recipe plugin then constrains that
-- symbolic pointer in a way that forces program execution into a state that
-- was negotiated with the CGC framework.

add_plugin("Recipe")
pluginsConfig.Recipe = {
    recipesDir = "{{ recipes_dir }}",
    logLevel = "warn"
}

-------------------------------------------------------------------------------
-- This plugin monitors call sites, i.e., pairs of source-destination program
-- counters. It is useful to recover information about indirect control flow,
-- which is hard to compute statically.

add_plugin("CallSiteMonitor")
pluginsConfig.CallSiteMonitor = {
    dumpInterval = 5,
}


{% endif %}

-------------------------------------------------------------------------------
-- The screenshot plugin records a screenshot of the guest into screenshotX.png,
-- where XX is the path number. You can configure the interval here:
add_plugin("Screenshot")
pluginsConfig.Screenshot = {
    period = 5
}

{% if enable_cfi %}
-------------------------------------------------------------------------------
-- This plugin keeps a set of valid code targets, e.g, for call and jump
-- instructions. It is used by CFIChecker to verify the validity of call targets.
add_plugin("AddressTracker")

-------------------------------------------------------------------------------
-- This plugins implements control flow integrity checking. It verifies that
-- return instructions go to the return address set by the corresponding call.
add_plugin("CFIChecker")
{% endif %}

{% if enable_tickler %}
-------------------------------------------------------------------------------
-- The tickler works in conjunction with the guest tool tickler{32|64}.exe.
-- It is a program that monitors the execution for Microsoft Office, Adobe
-- Acrobat, or FoxitReader. The tickler attempts to interact with the UI,
-- e.g., scrolling documents, closing pop ups, etc. It terminates the analysis
-- when nothing of interest happens in the target application after a while.
add_plugin("Tickler")

pluginsConfig.Tickler = {
  monitorIdleAfterAutoscroll = true,
  {% if enable_cfi %}
  -- A non-zero value will cause the tickler to terminate the analysis when
  -- that number of violations is reached.
  maxCfiViolations = 0,
  {% endif %}
}
{% endif %}

-- ========================================================================= --
-- ============== Target-specific configuration begins here. =============== --
-- ========================================================================= --

-------------------------------------------------------------------------------
-- LinuxMonitor is a plugin that monitors Linux events and exposes them
-- to other plugins in a generic way. Events include process load/termination,
-- thread events, signals, etc.
--
-- LinuxMonitor requires a custom Linux kernel with S2E extensions. This kernel
-- (and corresponding VM image) can be built with S2E tools. Please refer to
-- the documentation for more details.

add_plugin("LinuxMonitor")
pluginsConfig.LinuxMonitor = {
    -- Kill the execution state when it encounters a segfault
    terminateOnSegfault = true,

    -- Kill the execution state when it encounters a trap
    terminateOnTrap = true,
}

{% if enable_pov_generation %}

-------------------------------------------------------------------------------
-- This plugin writes PoVs as input files. This is suitable for programs that
-- take their inputs from files (instead of stdin or other methods).
add_plugin("FilePovGenerator")
pluginsConfig.FilePovGenerator = {
    -- The generated PoV will set the program counter
    -- of the vulnerable program to this value
    target_pc = 0x0011223344556677,

    -- The generated PoV will set a general purpose register
    -- of the vulnerable program to this value.
    target_gp = 0x8899aabbccddeeff
}

{% endif %}

-- ========================================================================= --
-- ============== User-specific scripts begin here ========================= --
-- ========================================================================= --


-------------------------------------------------------------------------------
-- This plugin exposes core S2E engine functionality to LUA scripts.
-- In particular, it provides the g_s2e global variable, which works similarly
-- to C++ plugins.
-------------------------------------------------------------------------------
add_plugin("LuaBindings")

-------------------------------------------------------------------------------
-- Exposes S2E engine's core event.
-- These are similar to events in CorePlugin.h. Please refer to
-- the LuaCoreEvents.cpp source file for a list of availble events.
-------------------------------------------------------------------------------
add_plugin("LuaCoreEvents")

-- This configuration shows an example that kills states if they fork in
-- a specific module.
--[[
pluginsConfig.LuaCoreEvents = {
    -- This annotation is called in case of a fork. It should return true
    -- to allow the fork and false to prevent it.
    onStateForkDecide = "onStateForkDecide"
}

function onStateForkDecide(state)
   mmap = g_s2e:getPlugin("ModuleMap")
   mod = mmap:getModule(state)
   if mod ~= nil then
      name = mod:getName()
      if name == "mymodule" then
          state:kill(0, "forked in mymodule")
      end

      if name == "myothermodule" then
          return false
      end
   end
   return true
end
--]]

{{ custom_lua_string }}
