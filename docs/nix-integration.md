# Nix integration reference

SplitCast owns its packages and Nix modules. Consumers import the flake modules
and supply host-specific settings. The modules do not depend on a feature
framework, fixed hostnames, or fixed user accounts.

## Flake exports

The flake exposes `homeManagerModules.default` and `nixosModules.default`.
Their definitions use different import forms:

```nix
homeManagerModules.default = import ./nix/modules/home-manager.nix { inherit self; };
nixosModules.default = import ./nix/modules/nixos.nix;
```

The Home Manager module closes over the SplitCast flake's `self`. Consumers do
not need `specialArgs` or a package overlay. Its default package is
`self.packages.${pkgs.stdenv.hostPlatform.system}.default`.

## Home Manager agent

This configuration defines a production user agent but keeps its start command
migration-gated:

```nix
{
	imports = [ inputs.splitcast.homeManagerModules.default ];
	services.splitcast = {
		enable = true;
		role = "production";
		rustCandidate = false;
		settings.peer.ssh_host = "capture-host";
	};
}
```

The module defines these `services.splitcast` options:

| Option | Default | Meaning |
| --- | --- | --- |
| `enable` | `false` | Installs the package, generated TOML, and manual user unit. |
| `role` | `"capture"` | Either `"capture"` or `"production"`, one role per user. |
| `package` | Flake default package | One package for the agent and cleanup commands. |
| `rustCandidate` | `false` | Selects the experimental agent command instead of the failing migration gate. |
| `settings` | `{}` | Recursive overrides of the example configuration and role defaults. |
| `configFile` | Generated TOML path | Absolute configuration path used by both agent and cleanup. |
| `environment` | `{}` | Non-secret user-service environment variables. |

The package must expose `example`, `rnnoise`, and `obs` passthru attributes.
`example` is a JSON configuration file. The module converts its values to TOML,
sets package-derived RNNoise and OBS paths, and then applies `settings`.
`settings.role` must match `role`.

Capture defaults to the SSH alias `production-host`. Production defaults to
`capture-host`. Consumers can override `settings.peer.ssh_host` or define those
aliases in their SSH configuration. Production swaps the control ports to
17871 and 17870, uses forwarding port 17873, and swaps the control-token paths.
Other defaults come from `package.example`.

The generated file appears at `~/.config/splitcast/<role>.toml`. A `configFile`
override changes the commands' input but does not remove that generated file.
The runtime file's contents are not checked during Nix evaluation.

Secrets are never configuration values. Token, password, stream-key, and Twitch
credential settings contain paths to private files owned by the service user.
The defaults refer to files under `~/.config/splitcast`. The module does not
provision their contents. Nix settings and service environment values are
stored in the world-readable Nix store.

The unit is named `splitcast-capture.service` or `splitcast-production.service`.
It has no `WantedBy` target and does not start at login. With
`rustCandidate = false`, `ExecStart` reports the migration gate and exits with
failure. Changing the flag requires separate parity verification and explicit
cutover approval. Neither flag value proves media or hardware correctness.

The unit retains `KillMode=control-group`, a 20-second stop timeout,
`ExecStopPost` cleanup, `UMask=0077`, `CPUWeight=25`, `Nice=10`, and
`MemoryMax=2G`. Failed starts restart after five seconds, at most three times
per minute. Cleanup uses the selected package even when the start command is
migration-gated. A manual start of a gated unit can therefore still run cleanup.

## NixOS prerequisites

The NixOS module configures system prerequisites, not an agent or a user account.
Its `services.splitcast.enable` and `role` settings are independent of the Home
Manager settings:

```nix
{
	imports = [ inputs.splitcast.nixosModules.default ];
	services.splitcast = {
		enable = true;
		role = "production";
	};
}
```

When enabled, the module defaults RTKit, PipeWire, PulseAudio compatibility,
ALSA support, WirePlumber, and graphics support to enabled. It also installs
`v4l-utils`. Existing explicit consumer settings override these defaults.
The privileged `gpu-screen-recorder` helper defaults to enabled only when both
`enable = true` and `role = "capture"`. Importing the module or enabling only
the production role does not enable that helper.

A migration with capture disabled, production enabled, and
`rustCandidate = false` retains those states. The NixOS module does not infer
capture activation from another machine's production role.

## Optional NVIDIA display

`nixosModules.default` includes the options in
`nix/modules/headless-display.nix`. `services.splitcast.display.enable` defaults
to `false`, including for production. The optional display requires enabled
production prerequisites, an existing `display.user`, and the NVIDIA Xorg
driver in `services.xserver.videoDrivers`. NVIDIA package evaluation also
requires the consumer's unfree-package policy to allow that driver.

`display.display` defaults to 99 and accepts numbers from 1 through 255. The
display is a manual system unit named `splitcast-display.service`, with no
`wantedBy` target. Its Xorg configuration creates a 1920-by-1080 NVIDIA display
without a physical monitor and disables TCP listening. This is not a generic
software display or a change to the user's existing desktop session.

The user-readable Xauthority cookie is mode `0600` at
`/run/splitcast-display/Xauthority`. The runtime directory is mode `0711`.
The Home Manager agent's environment remains a separate consumer setting:

```nix
services.splitcast.environment = {
	DISPLAY = ":99";
	XAUTHORITY = "/run/splitcast-display/Xauthority";
};
```

There is no automatic dependency between the user agent and the system display.
The display's NVIDIA, Xorg, GPU ownership, and session compatibility require
physical-host verification before use.

## Optional stock-OBS firewall

`nixosModules.default` also includes
`nix/modules/stock-obs-firewall.nix`. The option
`services.splitcast.stockObsFirewall.enable` defaults to `false` independently
of the agent and display. With the option disabled, the module does not change
the firewall backend or install rules.

Enabling the option enables `networking.nftables` and can replace the current
firewall backend. The `inet` input rule drops incoming TCP on every
non-loopback interface for `stockObsFirewall.port`, which defaults to 4455.
The rule covers IPv4, IPv6, and VPN interfaces. Local SSH forwarding can still
reach the loopback listener, subject to the rest of the firewall policy.
The rule does not make a wildcard OBS listener a loopback-bound listener.

The guard is an alternative boundary for stock OBS, not a default replacement
for the package's patched OBS. Its port must match `settings.obs.websocket_port`
if that port is overridden. Backend conflicts, the effective ruleset, SSH
reachability, and rejection of remote WebSocket connections require verification
before deployment. No firewall or service deployment is part of module evaluation.
