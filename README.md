# challenge_nats

ANSI char only, no utf support

follow zero allocation parsing


## Lock-based - deadend
Using lock ended up in a dead end because the nature of the push-mechanism

Each client has to maintain connectivity, so server have to hold on into socket

When PUB command is executed, it needs to write to socket by other client while 
server is still holding on to the lock due to the infinite loop for other client
hence pub command got stuck, there's no way for it to obtain the lock unless
server release the lock - but it needs the lock for the infinite loop

Will have to rewrite to use  message passing approach