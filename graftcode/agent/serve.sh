#!/bin/sh
set -eu

here=$(cd "$(dirname "$0")" && pwd)
port=${QUINJET_AGENT_PORT:-7390}
gateway=${QUINJET_GRAFTCODE_GATEWAY:-gg}

cd "$here"
uv sync --locked --no-dev --quiet
python="$here/.venv/bin/python"
library=$("$python" -c 'import pathlib, sysconfig; print(pathlib.Path(sysconfig.get_config_var("LIBDIR")) / sysconfig.get_config_var("LDLIBRARY"))')
packages=$("$python" -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')

HYPERTUBE_PYTHON_RUNTIME_PATH=${HYPERTUBE_PYTHON_RUNTIME_PATH:-$library}
PYTHONPATH="$packages${PYTHONPATH:+:$PYTHONPATH}"
export HYPERTUBE_PYTHON_RUNTIME_PATH PYTHONPATH

exec "$gateway" --runtime python --modules ./quinjet_agent/ --types quinjet_agent.agent.Agent \
  --port "$port" --httpPort "$((port + 1))" --GSMU=0
