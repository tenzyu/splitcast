# Security boundaries

SplitCast control is loopback-only. Remote access uses authenticated SSH forwarding. The local HTTP API validates bearer tokens, Host, Origin, framing, and bounds. These checks do not make the API safe for public exposure.

Credential settings are paths, not secret values. Files must be bounded private regular files owned by the service user. Never put operational secrets in Nix settings, service arguments, Git, diagnostics, or issue reports. Do not publish full environment dumps or private recordings.

The packaged OBS patch preserves a loopback WebSocket boundary. Stock OBS plus firewall rules is a separate opt-in experiment. Authentication alone does not replace verified listener isolation and effective IPv4/IPv6 ingress policy.

The capture worker runs outside the game. Do not inject into the game, modify bundled libraries, or silently substitute desktop capture. Microphone name validation rejects known monitor and processed-source names; it does not prove actual device provenance.

Process groups do not contain descendants that deliberately escape their group and cannot clean up after controller SIGKILL. An outer service cgroup is required. Destructor cleanup is best effort, not deployment acceptance. See the design review for open shutdown and task-joining risks.

Public release does not imply a vulnerability-response service or a production support promise. Report security findings privately to the repository owner when a private contact becomes available; do not attach credentials or exploit private hosts. No contact address is invented here.
