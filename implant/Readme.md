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
