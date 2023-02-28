#!/bin/bash
#
# This file was automatically generated by s2e-env at {{ creation_time }}
#
# This bootstrap script is used to control the execution of the target program
# in an S2E guest VM.
#
# When you run launch-s2e.sh, the guest VM calls s2eget to fetch and execute
# this bootstrap script. This bootstrap script and the S2E config file
# determine how the target program is analyzed.
#

set -x

{% if project_type == 'windows' %}
cd /c/s2e
{% endif %}

mkdir -p guest-tools32
TARGET_TOOLS32_ROOT=guest-tools32

{% if image_arch=='x86_64' %}
mkdir -p guest-tools64
TARGET_TOOLS64_ROOT=guest-tools64
{% endif %}

{% if image_arch=='x86_64' %}
# 64-bit tools take priority on 64-bit architectures
TARGET_TOOLS_ROOT=${TARGET_TOOLS64_ROOT}
{% else %}
TARGET_TOOLS_ROOT=${TARGET_TOOLS32_ROOT}
{% endif %}


# To save the hassle of rebuilding guest images every time you update S2E's guest tools,
# the first thing that we do is get the latest versions of the guest tools.
function update_common_tools {
    local OUR_S2ECMD

    OUR_S2ECMD=${S2ECMD}

    # First, download the common tools
    {% if project_type == 'windows' %}
    # Windows does not allow s2ecmd.exe to overwrite itself, so we need a workaround.
    if echo ${COMMON_TOOLS} | grep -q s2ecmd; then
      OUR_S2ECMD=${S2ECMD}_old.exe
      mv ${S2ECMD} ${OUR_S2ECMD}
    fi
    {% endif %}

    for TOOL in ${COMMON_TOOLS}; do
        ${OUR_S2ECMD} get ${TARGET_TOOLS_ROOT}/${TOOL}
        if [ ! -f ${TOOL} ]; then
          ${OUR_S2ECMD} kill 0 "Could not get ${TOOL} from the host. Make sure that guest tools are installed properly."
          exit 1
        fi
        chmod +x ${TOOL}
    done
}

function update_target_tools {
    for TOOL in $(target_tools); do
        ${S2ECMD} get ${TOOL} ${TOOL}
        chmod +x ${TOOL}
    done
}

function prepare_target {
    # Make sure that the target is executable
    chmod +x "$1"
}

{% if project_type == 'windows' %}
  {% set RAMDISK_ROOT='x:\\' %}
{% else %}
  {% set RAMDISK_ROOT='/tmp/' %}
{% endif %}

function get_ramdisk_root {
  echo '{{RAMDISK_ROOT}}'
}

function copy_file {
  SOURCE="$1"
  DEST="$2"
  {% if project_type == 'windows' %}
  run_cmd "copy /Y ${SOURCE} ${DEST}" > /dev/null
  {% else %}
  cp ${SOURCE} ${DEST}
  {% endif %}
}

# This prepares the symbolic file inputs.
# This function takes as input a seed file name and makes its content symbolic according to the symranges file.
# It is up to the host to prepare all the required symbolic files. The bootstrap file does not make files
# symbolic on its own.
function download_symbolic_file {
  SYMBOLIC_FILE="$1"
  RAMDISK_ROOT="$(get_ramdisk_root)"

  ${S2ECMD} get "${SYMBOLIC_FILE}"
  if [ ! -f "${SYMBOLIC_FILE}" ]; then
    ${S2ECMD} kill 1 "Could not fetch symbolic file ${SYMBOLIC_FILE} from host"
  fi

  copy_file "${SYMBOLIC_FILE}" "${RAMDISK_ROOT}"

  SYMRANGES_FILE="${SYMBOLIC_FILE}.symranges"

  ${S2ECMD} get "${SYMRANGES_FILE}" > /dev/null

  # Make the file symbolic
  if [ -f "${SYMRANGES_FILE}" ]; then
     export S2E_SYMFILE_RANGES="${SYMRANGES_FILE}"
  fi

  {% if enable_pov_generation %}
  # It is important to have one symbolic variable by byte to make PoV generation work.
  # One-byte variables simplify input mapping in the Recipe plugin.
  ${S2ECMD} symbfile 1 "${RAMDISK_ROOT}${SYMBOLIC_FILE}" > /dev/null
  {% else %}
  # The symbolic file will be split into symbolic variables of up to 4k bytes each.
  ${S2ECMD} symbfile 4096 "${RAMDISK_ROOT}${SYMBOLIC_FILE}" > /dev/null
  {% endif %}
}

function download_symbolic_files {
  for f in "$@"; do
    download_symbolic_file "${f}"
  done
}

{% if use_seeds %}
# Our msys distribution doesn't have seq, so this is a quick replacement
function my_seq {
  START=$1
  END=$2
  while [ $START -le $END ]; do
    echo $START
    START=$(expr $START + 1)
  done
}

function execute {
  RAMDISK_ROOT="$(get_ramdisk_root)"

  # In seed mode, state 0 runs in an infinite loop trying to fetch and
  # schedule new seeds. It works in conjunction with the SeedSearcher plugin.
  # The plugin schedules state 0 only when seeds are available.

  # Enable seeds and wait until a seed file is available. If you are not
  # using seeds then this loop will not affect symbolic execution - it will
  # simply never be scheduled.
  ${S2ECMD} seedsearcher_enable
  while true; do
      SEED_FILE=$(${S2ECMD} get_seed_file)

      if [ $? -eq 1 ]; then
          # Avoid flooding the log with messages if we are the only runnable
          # state in the S2E instance
          sleep 1
          continue
      fi

      break
  done

  if [ -n "${SEED_FILE}" ]; then
      download_symbolic_file "${SEED_FILE}"

      # Patch the default symbolic file with the path to the seed
      ARGS=("${@}")
      SEED_FILE_PATH="${RAMDISK_ROOT}${SEED_FILE}"
      for i in $(my_seq 0 $(expr ${#ARGS[*]} - 1)); do
        if [ ${ARGS[$i]} = "${RAMDISK_ROOT}input-0" ]; then
          ARGS[$i]="${SEED_FILE_PATH}"
        fi
      done

      echo "${ARGS[@]}"
      execute_target "${ARGS[@]}"
  else
      # If there are no seeds available, execute the seedless instance.
      # The SeedSearcher only schedules the seedless instance once.
      echo "Starting seedless execution"

      {% if project_type == 'cgc' %}
      execute_target "$1"
      {% else %}
      # NOTE: If you do not want to use seedless execution, enable the following line and remove
      # the state kill command.
      # execute_target "$1"
      ${S2ECMD} kill 0 "Target invocation without seed file is disabled. Please edit bootstrap.sh to enable it."
      {% endif %}
  fi
}

{% else %}

# This function executes the target program given in arguments.
#
# There are two versions of this function:
#    - without seed support
#    - with seed support (-s argument when creating projects with s2e_env)
function execute {
    local TARGET

    TARGET="$1"
    shift

    execute_target "${TARGET}" "$@"
}

{% endif %}

###############################################################################
# This section contains target-specific code

{% include '%s' % target_bootstrap_template %}

###############################################################################


update_common_tools
update_target_tools

{% if project_type != 'windows' %}

# Don't print crashes in the syslog. This prevents unnecessary forking in the
# kernel
sudo sysctl -w debug.exception-trace=0

# Prevent core dumps from being created. This prevents unnecessary forking in
# the kernel
ulimit -c 0

# Ensure that /tmp is mounted in memory (if you built the image using s2e-env
# then this should already be the case. But better to be safe than sorry!)
if ! mount | grep "/tmp type tmpfs"; then
    sudo mount -t tmpfs -osize=10m tmpfs /tmp
fi

# Need to disable swap, otherwise there will be forced concretization if the
# system swaps out symbolic data to disk.
sudo swapoff -a

{% endif %}

target_init

# Download the target file to analyze
{% for tf in target.file_names_to_s2eget -%}
${S2ECMD} get "{{ tf }}"
{% endfor %}

{% if not use_seeds %}
download_symbolic_files {{ target.args_space_concatenated_symbolic_file_names }}
{% endif %}

{% if target %}
# Run the analysis

{% if project_type == 'windows' %}
  TARGET_PATH='{{ target.name }}'
{% else %}
  TARGET_PATH='./{{ target.name }}'
{% endif %}


prepare_target "${TARGET_PATH}"

{% if use_seeds %}
# In seed mode, the symbolic file name is a place holder that will be replaced by the actual seed name.
{%- endif %}

{% if enable_tickler %}
TARGET_PATH_WITH_SLASHES="$(echo ${TARGET_PATH} | sed 's:\\:/:g')"
BINARY_NAME="$(basename "${TARGET_PATH_WITH_SLASHES}")"
if [ "x${BINARY_NAME}" = "xwinword.exe" ]; then
  execute tickler.exe MsWord
elif [ "x${BINARY_NAME}" = "xexcel.exe" ]; then
  execute tickler.exe MsExcel
elif [ "x${BINARY_NAME}" = "xpowerpnt.exe" ]; then
  execute tickler.exe MsPowerPoint
fi
{% endif %}

execute "${TARGET_PATH}" {{ target.args_space_concatenated_all }}

{% else %}
##### NO TARGET HAS BEEN SPECIFIED DURING PROJECT CREATION #####
##### Please fetch and execute the target files manually   #####
{% endif %}
