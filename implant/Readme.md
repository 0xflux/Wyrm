# Wyrm agent

The Wyrm agent is a post exploitation Red Team framework designed to operate as a RAT.

## How it works

### Command and control

The agent communicates with the C2 over HTTP(S); future support is planned for C2 over DNS.

When the implant is first run, it will make a first call home indicating that it has started for the first time,
allowing it to get any configuration information from the C2, such as its sleep time, or other malleable settings.

Following this, the agent enters the C2 loop in which GET requests are made to the C2 and tasks are received, executed
then returned the output via a POST request.

During transit, the comms are encrypted with a simple XOR scheme. Given SSL inspection will not be brute forcing comms traffic
below TLS; simple XOR is deemed sufficient complex for the threat model of red teams.

**Note:** Extra care has been made to ensure that artifacts of the messaging structures are not left over and present 
in the binary which could become searchable strings.

## Design documentation

A little documentation to explain some of the design decisions. It feels like spaghetti in parts, so this is putting my
brain on paper for now to explain some of the core concepts around how the implant works.

When the implant first runs; it will conduct a 'first run' function to send some environment data up to the C2. This function
is `first_check_in`, and will try a number of times (as per what it is configured with) to POST this data up to the C2.

Following this, the agent enters its **C2 loop**.

### C2 loop

The C2 loop can be thought as a massive dispatcher, which is dispatching a `shared::tasks::Command`.

Each `Command` which will be dispatched (in `dispatch_tasks`) should call `self.push_completed_task()` to push the result
of some task to be dispatched to the implants `completed_tasks` Vec.

Due to custom serialisation (OPSEC strategy), `push_completed_task` will serialise the result of the function appropriately,
encoding data into the packet structure. It will produce this as a Vector of u16 (to allow for unicode characters)
and this is pushed to `completed_tasks` which is ultimately, a `Vec<Vec<u16>>`.

**For this reason**, what goes into `push_completed_task` is an `Option<impl Serialize>`. Thus, it follows, any function which
is used in the main dispatcher, itself should return `Option<impl Serialize>`. To avoid issues with references,
you may need to return `Option<impl Serialize + use<>>` (if it moans about some move semantics / ownership rules).

A final note: only tasks which you wish to POST back to the C2 to 'complete' them (in the `completed_tasks` db table)
need completing as above. There is no requirement to do this for tasks that you do not want feedback on, or need
the additional `completed_tasks` modifying.

To that end; tasks which are 'autocomplete' on pickup when the C2 grabs the tasks from the pending task queue, there is
an implementation on `shared::tasks::Command`, for the method `is_autocomplete`. Marking a discriminant as `true` will
allow the C2 to silently mark everything as completed in the db on the backend, so as soon as the agents requests new 
tasks, at that point it will be marked as complete and sent to the agent.

#### Errors within returned data

If your `Option<impl Serialize>` contains something you wish to express as an `Error`, I have provided the `WyrmError` enum,
which matches the signature of a standard `Result<T, E>` - except that it is represented:

```Rust
#[derive(Serialize, Deserialize)]
pub enum WyrmResult<T: Serialize> {
    Ok(T),
    Err(String),
}
```

This allows you to return the `WyrmResult` and have it serialise inside of a `Some()`, for example:

**Ok**

```Rust
result = Some(
    WyrmResult::Ok(self.current_working_directory
        .to_string_lossy()
        .into_owned(),
));
```

**Err**

```Rust
let return_value = match e.kind() {
    std::io::ErrorKind::NotFound => Some(WyrmResult::Err("Not found".to_string())),
    std::io::ErrorKind::PermissionDenied => Some(WyrmResult::Err("Permission denied.".to_string())),
    _ => Some(WyrmResult::Err(format!("An error occurred. Code: {}", e.raw_os_error().unwrap_or_default()))),
};
```

Then on the client, you can display these easily such as (in `shared_c2_client`):

```Rust
let deser: WyrmResult<PathBuf> = match serde_json::from_str(result) {
    Ok(d) => d,
    Err(e) => {
        print_client_error(&msg_header, &format!("Ensure your request was properly formatted: {e}"));
        return;
    },
};
match deser {
    WyrmResult::Ok(result) => println!("{}{}", msg_header, result.display()),
    WyrmResult::Err(e) => print_client_error(&msg_header, &e),
}
```

## Binary size

A note on the binary size, currently building ~ 1.4 MB, which is acceptable, but I would like it to be significantly leaner. From
the `cargo bloat` below, the biggest offenders are going to be `rustls`, the `minreq` and `serde`. Removing these may remove
some significant bloat. The networking libs will be a little cumbersome to implement, but can be done with winhttp. 

Removing `minreq` results in a ~350 kb binary, which is much more in line with expectations.

Output of running `cargo bloat`:

```shell
# Note turn off debug symbol stripping, etc
cargo bloat --release --features sandbox_mem,sandbox_trig,patch_etw -n 50

 File  .text     Size   Crate Name
 1.4%   2.1%  19.4KiB     std std::sys::process::windows::Command::spawn_with_attributes
 0.7%   1.1%  10.1KiB implant implant::wyrm::Wyrm::dispatch_tasks
 0.7%   1.1%   9.9KiB  rustls rustls::client::hs::emit_client_hello_for_retry
 0.6%   0.9%   8.9KiB  rustls rustls::enums::impl$40::fmt
 0.6%   0.9%   8.9KiB  rustls rustls::enums::impl$40::fmt
 0.6%   0.9%   8.2KiB  rustls <rustls::client::tls12::ExpectServerDone as rustls::common_state::State<rustls::client::client_conn::ClientCo...
 0.5%   0.8%   7.3KiB  rustls <rustls::client::tls13::ExpectFinished as rustls::common_state::State<rustls::client::client_conn::ClientConn...
 0.5%   0.7%   6.6KiB  shared <&mut serde_json::de::Deserializer<R> as serde_core::de::Deserializer>::deserialize_struct
 0.4%   0.6%   6.1KiB  minreq rustls::conn::ConnectionCommon<Data>::complete_io
 0.4%   0.6%   5.6KiB  rustls <rustls::client::hs::ExpectServerHelloOrHelloRetryRequest as rustls::common_state::State<rustls::client::clie...
 0.4%   0.6%   5.2KiB  rustls <rustls::client::hs::ExpectServerHello as rustls::common_state::State<rustls::client::client_conn::ClientConn...
 0.4%   0.5%   5.0KiB     std core::num::flt2dec::strategy::dragon::format_shortest
 0.3%   0.5%   4.8KiB     std alloc::collections::btree::map::BTreeMap::insert<std::sys::process::windows::EnvKey,std::ffi::os_str::OsStrin...
 0.3%   0.5%   4.8KiB  rustls rustls::msgs::handshake::HandshakeMessagePayload::read_version
 0.3%   0.5%   4.6KiB  rustls rustls::client::tls12::server_hello::CompleteServerHelloHandling::handle_server_hello
 0.3%   0.5%   4.5KiB  minreq minreq::response::ResponseLazy::from_stream
 0.3%   0.5%   4.5KiB  rustls rustls::client::hs::start_handshake
 0.3%   0.5%   4.4KiB  rustls <rustls::client::tls13::ExpectEncryptedExtensions as rustls::common_state::State<rustls::client::client_conn:...
 0.3%   0.4%   4.2KiB implant implant::native::filesystem::pillage
 0.3%   0.4%   4.1KiB     std core::num::flt2dec::strategy::dragon::format_exact
 0.3%   0.4%   4.1KiB  rustls rustls::client::tls13::handle_server_hello
 0.3%   0.4%   3.8KiB implant implant::wyrm::Wyrm::new
 0.3%   0.4%   3.8KiB  rustls <rustls::client::tls12::ExpectFinished as rustls::common_state::State<rustls::client::client_conn::ClientConn...
 0.3%   0.4%   3.8KiB  rustls <rustls::msgs::handshake::HandshakeMessagePayload as rustls::msgs::codec::Codec>::encode
 0.3%   0.4%   3.7KiB    ring ring::ec::suite_b::ops::p384::p384_elem_inv_squared
 0.3%   0.4%   3.6KiB implant implant::native::filesystem::parse_path
 0.3%   0.4%   3.6KiB    ring ring_core_0_17_14__x25519_sc_reduce
 0.2%   0.4%   3.4KiB  rustls <rustls::verify::WebPkiVerifier as rustls::verify::ServerCertVerifier>::verify_server_cert
 0.2%   0.4%   3.3KiB  minreq minreq::connection::handle_redirects
 0.2%   0.3%   3.3KiB  minreq minreq::request::ParsedRequest::redirect_to
 0.2%   0.3%   3.3KiB         ge_double_scalarmult_vartime
 0.2%   0.3%   3.2KiB     std std::sys::process::env::CommandEnv::capture_if_changed
 0.2%   0.3%   3.2KiB  rustls rustls::msgs::deframer::MessageDeframer::pop
 0.2%   0.3%   3.2KiB  webpki webpki::verify_cert::build_chain_inner
 0.2%   0.3%   3.2KiB     std std::path::PathBuf::_push
 0.2%   0.3%   3.2KiB  minreq minreq::connection::handle_redirects
 0.2%   0.3%   3.0KiB  rustls rustls::limited_cache::LimitedCache<K,V>::get_or_insert_default_and_edit
 0.2%   0.3%   3.0KiB  rustls <rustls::client::tls13::ExpectCertificateVerify as rustls::common_state::State<rustls::client::client_conn::C...
 0.2%   0.3%   3.0KiB    ring ring::ec::suite_b::ops::p384::p384_scalar_inv_to_mont
 0.2%   0.3%   3.0KiB  rustls <rustls::client::tls12::ExpectServerKx as rustls::common_state::State<rustls::client::client_conn::ClientConn...
 0.2%   0.3%   2.8KiB     std alloc::str::<impl str>::to_lowercase
 0.2%   0.3%   2.8KiB  rustls <rustls::client::tls12::ExpectCertificate as rustls::common_state::State<rustls::client::client_conn::ClientC...
 0.2%   0.3%   2.7KiB  minreq std::sync::mpmc::list::Channel<T>::recv
 0.2%   0.3%   2.7KiB  minreq hashbrown::map::HashMap<K,V,S,A>::insert
 0.2%   0.3%   2.7KiB  rustls <rustls::client::tls13::ExpectTraffic as rustls::common_state::State<rustls::client::client_conn::ClientConne...
 0.2%   0.3%   2.7KiB  rustls <core::iter::adapters::cloned::Cloned<I> as core::iter::traits::iterator::Iterator>::fold
 0.2%   0.3%   2.6KiB  rustls <rustls::client::tls12::ExpectCertificateStatus as rustls::common_state::State<rustls::client::client_conn::C...
 0.2%   0.3%   2.6KiB    ring ring::ec::suite_b::ops::p256::p256_scalar_inv_to_mont
 0.2%   0.3%   2.5KiB     std alloc::collections::btree::node::Handle::remove_leaf_kv<std::sys::process::windows::EnvKey,std::ffi::os_str::...
 0.2%   0.3%   2.5KiB  rustls <rustls::client::tls13::ExpectCertificateRequest as rustls::common_state::State<rustls::client::client_conn::...
47.6%  72.1% 675.1KiB         And 1771 smaller methods. Use -n N to show more.
66.1% 100.0% 937.0KiB         .text section size, the file size is 1.4MiB
```