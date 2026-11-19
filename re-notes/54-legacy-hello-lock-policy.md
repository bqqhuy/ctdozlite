# Legacy Hello-only app lock — 2026-11-19

Fork local privacy policy; no protocol/server change. MAP-00 requires app lock
to be opt-in, defaults disabled, and Windows Hello to be optional. Old saved
settings can still contain enabled lock + Hello without an app PIN.

The startup path previously tested asynchronous biometric availability, which
starts false. A restored session with those legacy settings therefore opened
unlocked regardless of the saved lock choice. The lock policy now depends on
configured PIN/Hello, while the availability probe only controls whether the
Hello unlock button is offered. Probe pending/failure/timeout never unlocks.

Default lock/Hello remain disabled, and startup does not automatically invoke
Hello. The existing lock-screen Logout command remains available for re-login
if Hello cannot be used; the subtitle explains this route for legacy no-PIN
settings. New lock setup still requires the existing PIN workflow.

Offline tests deserialize legacy settings without reading/writing real settings
and check restored lock persistence through probe pending/failure/timeout. Native
Hello, recovery re-login and UX require Windows device acceptance; no new E1
or pixel-parity claim follows from these tests.
