### Token Management

token usage keeps adding up, truncation might not work correctly, or rather the token calculation is not properly reflected

### What Happens when LLM Backend Errors out

do we set the correct turn and proper agent event? and do we handle the turn properly for errors?

I should try this with proper proxy

### Subagents

Subagents should only show visually the tool calls, not agents repsonses and thinking

### Compression

broken

### Permission.json

if hoosh is running, a change on permissions file is overwritten

### ctrl+c on setup and init_permission 

just enters instead of exiting

### Pipe to file should trigger permissions

echo "Hello, this is a test file created at $(date)" > test_output.txt && c..

did not

### tool calling fixing in case of crashes

currently adding a tool message with some answer -> better just remove the ones that dont have proper answer

### bash permission
heredoc keeps asking

### Auto Scroll 
auto scrolling when dialogs open up in custom terminal has a limitation, lets see if we can remove that height limit

### Explore and Plan agents are timing out


### LLM Keeps cd-ing in working directory


### Permission Dialog when exploring
currently does not pause the timer -> we have the methods in execution budget we should pause the timer, when user is in control
