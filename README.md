# challenge_nats
Attempt to write a NATS message broker in Rust (compiled using Rust 1.81)

See [original challenge link](https://codingchallenges.fyi/challenges/challenge-nats/)
for more details

## To build

```
cargo build --release
```

Default binary output will be stored in `./target/release/challenge_nats`

To execute

```
target/release/challenge_nats config.toml
```

## To run servers

```
script/servers.sh
```

This will launch 3 servers serving script/server1, script/server2, script/server3

## To run nats in dev mode

```
cargo run <config.toml>
```

If config is not provided, it defaults to `./config.toml`

## To test
After nats is running, either use `nats bench` or `netcat`

### Use netcat
```
nc -v localhost 4222
```

Now it act similar with telnet, you can then send the commands
```
CONNECT {}
PING
SUB subject id
PUB subject 5
noice
```

### Use nats bench
Set up subscriber
```
nats bench coding.challenge --sub 10 --msgs 10000
```

Set up publisher
```
nats bench coding.challenge --pub 10 --size 16 --msgs 10000
```

OR browse to http://localhost:8080

## Initial failed approach
Initially I attempted to use locks all the way but this ends up with dead end 
because of the nature of push-mechanism combined with Rust' ownership/lifetime

Because of push-mechanism, each client has to maintain connectivity so server 
has to hold on into the tcp socket

When PUB command is executed, it needs to write to socket that subscribed to 
the subject. The issue is now that the other client handler is holding on to 
the lock because the handler needs to continuously read from the socket

As a result, PUB command got stuck as there is no way to obtain the lock 
unless server releases the lock since writing into the socket requires 
mutability

The other way to get around this is to rewrite it and use message passing 
approach

## Design
Code are split into 5 main parts, namely
- main: main loop, handling graceful shutdown
- commands: processing MainCommand
- handlers: responsible for handling request and response
- parser: parsing client requests
- server: for the server struct, also as the main point to handle MainCommand

Command parsing follows the original approach with [zero allocation byte parser](https://github.com/nats-io/nats-server/blob/45e6812d70e42891ea2ff57e0a9a6051fa5a1d27/server/parser.go#L134)

Overall the high level overview is as follow:
- initially spawn a new task reading from main_rx channel. Task is handled 
  by command.process_rx
- next, the main will listen for the incoming client connection, spawning new 
  task as new client connection coming in
- handlers then responsible to handle the client connection, parses the 
  incoming message and transform it into ClientCommand
- handlers also creates client specific channel
- handlers further process ClientCommand and transform it into MainCommand
- handlers then send the message into main_tx channel
- command.process_rx receives MainCommand message, and call the process function
- when processing a PUB command, it will then finds the subscribers, and 
  obtained the client channels and sends a new MainCommand::PublishedMessage
  command 
- handlers also listen for MainCommand but only for PublishedMessage and 
  ShutDown in the client channel. Should it receive PublishedMessage, it will 
  finally write the MSG response into the socket

## Challenges
I faced quite a number of challenges when working with this
- async + lifetime/ownership makes things much harder
- for `PUB` command, somehow `nats bench` and `nats-server` only expects `\r` 
  to finish message body. This causes a big headache because I was initially 
  expecting `\n` for all commands including `PUB`
- previously didn't think that multiple commands can be in 1 go. Request 
  buffer is set at 4KB and there can be more than 1 commands
  (e.g. `PING` and `PUB`)
- handling message size that is larger than the request buffer
- didn't realise that INFO should include `max_payload`. Initially I didn't 
  include this and `nats bench` keep failing
- thought that verbose response was the default behaviour, only to find 
  out when it fails `nats bench` because it wasn't expecting `+OK` response
