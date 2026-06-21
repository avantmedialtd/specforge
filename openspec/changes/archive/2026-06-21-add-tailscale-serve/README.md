# add-tailscale-serve

Minimal, safe Tailscale Serve support for the local web UI: trust the node's own tailnet name in the web server's Host/Origin guard so tailscale serve can proxy to it, keeping the loopback bind and CSRF protection; off by default.
