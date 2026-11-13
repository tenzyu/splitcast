# Nix module evidence, 2026-11-13

## Scope and artifact identity

The completed checks exercised the four files in `nix/modules` directly with
real Home Manager and NixOS module evaluators. They did not exercise the new
root flake's package default or exported-module wiring.

Every Home Manager evaluation supplied a synthetic `self.packages` default.
The initial unit probes used `hello-2.12.3` with synthetic passthru attributes.
The TOML build used `pkgs.emptyDirectory`. The final command test used a
`writeShellScriptBin` executable that printed its arguments, with
`pkgs.emptyDirectory` as the RNNoise and OBS passthru values. None of those
packages ran the real SplitCast, OBS, or RNNoise implementation.

The synthetic packages read `packages/splitcast-rs/tests/example.json` for
`package.example`. The input sources were the locally resolved nixpkgs source
`7sa2x9c9566lfxkaxs36n3z4yayl5cy4-source`, reporting version `26.05pre-git`,
and Home Manager source `mq6k123g378wgwcz88s7zdmbwf69cxdy-source`, revision
`6eb2d33d1d0c95a37ef6443c88e6c1686012ee8b`.

These SHA-256 values identify the accepted module files and example fixture.
The hashes were collected after the completed tests, during evidence capture:

```text
0839ba13c7b5e584ff2872daf9b442f5dd09cc8f9fe6a902bdb93a981232e000  nix/modules/home-manager.nix
ffae35e20e1c5ae2f1b5258eff5b3b5fd506165ee03729f4c4aead05a721b8a1  nix/modules/nixos.nix
406af6ccf000ea2fb7f4c5d4e55efe60e2899be71e1c7525550b1b721f5245ec  nix/modules/headless-display.nix
f8310b19d9a0df0342d27ac2eef907badb41bb8fa6bcffc26502cd0361e45c8e  nix/modules/stock-obs-firewall.nix
1816132d7b6031aa17d72eda0fa320bd7902024699e16ddfd15510aabf6bcf84  packages/splitcast-rs/tests/example.json
```

## Commands actually run

The original probes were inline Python scripts in the agent session, not saved
test files. Their Nix subprocess argument vectors were:

```text
nix-instantiate --eval --strict --json --expr <inline expression>
nix-build --no-out-link --expr <inline expression>
```

The scripts used `subprocess.run(..., text=True, capture_output=True)` for Nix,
parsed evaluator output with `json.loads`, and checked results with Python
assertions. TOML checks used `tomllib.loads` on the built files. Command checks
used `subprocess.run([executable], capture_output=True)` and compared NUL-delimited
arguments. The final display probe used the shell prefix
`NIXPKGS_ALLOW_UNFREE=1 python3 - <<'PY'`.

Inline expressions contained local source paths. Those paths are omitted from
this record. `nix/tests.nix` preserves the test cases with portable inputs, not
a byte-for-byte copy of the historical scripts. The rerun commands below have
not been substituted for the historical commands.

The syntax check ran successfully before and after the wrapper change:

```sh
for module in nix/modules/*.nix; do nix-instantiate --parse "$module" >/dev/null || exit; done
```

It returned exit status 0 with no output. A source scan also returned no matches
for framework dependencies, a duplicate Rust package option, personal names,
machine-specific names, or literal Nix store paths in the module source.

## Passed probes and captured output

The corrected initial Home Manager probe printed:

```text
PASS: real Home Manager disabled/gated/candidate units, no WantedBy, cleanup, resource limits, single package
PASS: relative config, role disagreement, and invalid role fail evaluation with expected errors
```

The NixOS role and firewall probe printed:

```text
PASS: real NixOS disabled/production/capture gating, baseline prerequisites, display/firewall default off, opt-in dual-stack firewall with custom port
```

The TOML build probe printed:

```text
PASS: built and parsed actual capture, production, and overridden TOML at /nix/store/ji0jk311x84w8lkrc0rg1bi05v8xxd90-splitcast-module-config-tests
```

That probe checked role defaults, peer aliases, production control ports and
token paths, capture defaults, recursive overrides, and package-derived OBS
and RNNoise paths. It ran before the final command-wrapper change and the
equivalent restructuring of the role-default attribute set. The TOML build
was not repeated after that restructuring.

The corrected display and explicit-override probe printed:

```text
PASS: manual NVIDIA display unit, private cookie, TCP disabled, three prerequisite assertions, explicit system defaults remain overridable
```

After the final command-wrapper change, the Home Manager and synthetic
executable probes printed:

```text
PASS: final Home Manager disabled/gated/candidate units, no WantedBy, cleanup, limits, single package
PASS: final negative config/role assertions
PASS: built migration gate exits 1; agent/cleanup wrappers preserve spaces, quotes, dollar signs, and systemd percent specifiers with a synthetic binary
```

## Failed attempts and corrections

These failures were not omitted from the outcome:

| Attempt | Observed failure | Correction and result |
| --- | --- | --- |
| Dependency discovery through the source checkout's `getFlake` | An unrelated Unix socket had an unsupported file type. | Read the existing Home Manager lock entry and resolved it with `builtins.fetchTree` in offline mode. That dependency lookup passed. |
| First Home Manager Python probe | `AssertionError` showed `ExecStart` as a one-element list, not a string. | Indexed element zero. The corrected probe passed. |
| First enabled-display evaluation | Nix refused `nvidia-x11-595.71.05` because of its unfree license. | Set `NIXPKGS_ALLOW_UNFREE=1` for the test process only. No consumer policy changed. |
| Next display evaluation | `error: attribute 'cycle' missing` while forcing a successful filesystem assertion's diagnostic message. | Filtered failed assertions before reading messages. The corrected display probe passed. |

The deliberately invalid configuration cases failed with the expected errors
for a relative `configFile`, disagreement between the two role settings, and
an invalid role enum. Those expected failures count as passing negative tests.

## Evidence locations

The raw command output was retained in the agent conversation only. No raw
log files or temporary test scripts were saved. This document preserves the
reported output and limitations.

The following build outputs were found during evidence capture:

```text
/nix/store/ji0jk311x84w8lkrc0rg1bi05v8xxd90-splitcast-module-config-tests
/nix/store/5dy98bazwnadf0rhdyl6xapp89idjxgi-splitcast-module-command-tests
```

The first contains `capture.toml`, `production.toml`, and `overridden.toml`.
The second contains `gated`, `agent`, and `cleanup` executable links. These are
local, garbage-collectable store outputs, not committed logs or permanent
evidence URLs. The evidence-capture commands were:

```sh
printf '%s\n' /nix/store/*splitcast-module-command-tests /nix/store/*splitcast-module-config-tests
sha256sum nix/modules/home-manager.nix nix/modules/nixos.nix nix/modules/headless-display.nix nix/modules/stock-obs-firewall.nix packages/splitcast-rs/tests/example.json
```

## Portable evaluator and artifact commands

The new `nix/tests.nix` takes `{ nixpkgs, homeManager, self }`. It imports the
module files directly and injects a synthetic package default. It deliberately
does not evaluate the real package default or the exported-module wiring.
Its `evaluation` output includes individual Boolean results and `passed`.
Its `configs` and `commands` outputs are buildable synthetic fixtures.

The following commands are rerun instructions, not completed checks from this
record. `HOME_MANAGER_SRC` names an available Home Manager source directory.
The standalone flake does not currently declare a Home Manager input.

```sh
export HOME_MANAGER_SRC
NIX_TESTS='let
  flake = builtins.getFlake (toString ./.);
in import ./nix/tests.nix {
  nixpkgs = flake.inputs.nixpkgs;
  homeManager = builtins.toPath (builtins.getEnv "HOME_MANAGER_SRC");
  self = flake;
}'
nix eval --impure --json --expr "($NIX_TESTS).evaluation" > module-evaluation.json
python3 -c 'import json; assert json.load(open("module-evaluation.json"))["passed"]'
CONFIGS=$(nix build --impure --no-link --print-out-paths --expr "($NIX_TESTS).configs")
COMMANDS=$(nix build --impure --no-link --print-out-paths --expr "($NIX_TESTS).commands")
EXPECTED_PATH=$(nix eval --impure --raw --expr "($NIX_TESTS).expectedConfigPath")
export CONFIGS COMMANDS EXPECTED_PATH
python3 - <<'PY'
import os
import subprocess
import tomllib
from pathlib import Path

root = Path(os.environ["CONFIGS"])
capture, production, overridden = [
    tomllib.loads((root / f"{name}.toml").read_text())
    for name in ["capture", "production", "overridden"]
]
assert capture["role"] == "capture"
assert production["role"] == "production"
assert capture["peer"]["ssh_host"] != production["peer"]["ssh_host"]
assert [production["peer"][key] for key in [
    "control_port", "remote_control_port", "forward_control_port"
]] == [17871, 17870, 17873]
assert production["peer"]["token_file"] == "~/.config/splitcast/production.token"
assert production["peer"]["remote_token_file"] == "~/.config/splitcast/control.token"
assert capture["peer"]["control_port"] == 17870
assert capture["capture"]["width"] == 1280
assert overridden["capture"]["width"] == 1920
assert overridden["peer"]["ssh_host"] not in [
    capture["peer"]["ssh_host"], production["peer"]["ssh_host"]
]
assert capture["audio"]["rnnoise_plugin"].endswith("/lib/ladspa/librnnoise_ladspa.so")
assert capture["obs"]["binary"].endswith("/bin/obs")
root = Path(os.environ["COMMANDS"])
result = subprocess.run([root / "gated"], capture_output=True)
assert result.returncode == 1 and b"pending Rust parity" in result.stderr
for command in ["agent", "cleanup"]:
    result = subprocess.run([root / command], capture_output=True)
    assert result.returncode == 0
    assert result.stdout.split(b"\0") == [
        command.encode(), b"--config", os.fsencode(os.environ["EXPECTED_PATH"]), b""
    ]
print("PASS: synthetic module TOML and command artifacts")
PY
```

The portable harness changes the synthetic account, runtime path, and package
fixture from the historical expressions. Its enabled-display fixture permits
unfree packages inside that test evaluation only. These are reproduction
fixtures, not production settings.

## Evidence-capture syntax checks

Only syntax checks ran against the new reproduction files:

```sh
nix-instantiate --parse nix/tests.nix >/dev/null
```

That command returned exit status 0 with no output. The following command also
returned exit status 0:

```sh
python3 - <<'PY'
import ast
from pathlib import Path
text = Path('docs/evidence/nix-modules-20261113.md').read_text()
source = text.split("python3 - <<'PY'\n", 1)[1].split('\nPY\n', 1)[0]
ast.parse(source)
print('PASS: documented portable Python verifier parses; not executed')
PY
```

Its output was:

```text
PASS: documented portable Python verifier parses; not executed
```

## Skipped and unproven

- The new harness has not been evaluated or built in this evidence-capture pass.
- The root package, root flake exports, and consuming configurations remain
  outside this record's tested artifact identity.
- No real agent, OBS, RNNoise, capture helper, display, or network listener ran.
- No effective firewall ruleset, SSH reachability, GPU ownership, media behavior,
  credential provisioning, or physical-device compatibility was tested.
- No deployments, service starts, Cargo checks, or heavy-check reruns occurred
  during evidence capture. Migration and opt-in gates remain unchanged.
