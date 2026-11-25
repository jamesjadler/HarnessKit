# One coordinator with remote access

Use one coordinating HarnessKit instance for environment management and skill
synchronization, preferring SSH over Tailscale and Windows host access to WSL
over a HarnessKit service in every environment. This honors the preference to
avoid per-environment installations, accepting that sync pauses when the
coordinator is unavailable; remote transport feasibility still needs validation.

T3 integration follows standalone environment support rather than being a runtime
dependency, so the manager does not depend on a future T3 plugin interface.
