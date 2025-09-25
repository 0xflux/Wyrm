# Shared

This crate holds shared types, implementations, and logic which are shared across multiple crates,
but importantly, things which will not lead to OPSEC leaks on the release build of the agent.

For anything which may cause OPSEC problems, or type problems due to OPSEC strategy, see
the sibling crate, `shared_c2_client`.