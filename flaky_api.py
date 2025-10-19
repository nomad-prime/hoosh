"""
mitmproxy addon for simulating flaky LLM APIs
Usage: mitmproxy -s flaky_llm.py --set failure_rate=0.3
"""
import random
from mitmproxy import http, ctx

class FlakyLLM:
    def __init__(self):
        self.request_count = 0
        
    def load(self, loader):
        loader.add_option(
            name="failure_rate",
            typespec=str,
            default="0.3",
            help="Probability of injecting a failure (0.0 to 1.0)",
        )
        
    def request(self, flow: http.HTTPFlow) -> None:
        # Only intercept LLM API calls
        if "together.xyz" in flow.request.host or "anthropic.com" in flow.request.host:
            self.request_count += 1
            
            # Get failure rate from options and convert to float
            failure_rate = float(ctx.options.failure_rate)
            
            if random.random() < failure_rate:
                print(f"🔴 Injecting failure for request #{self.request_count}")
                
                # Randomly choose failure type
                failure_type = random.choice(['rate_limit', 'server_error', 'network_error'])
                
                if failure_type == 'rate_limit':
                    flow.response = http.Response.make(
                        429,
                        b'{"error": {"message": "Rate limit exceeded", "type": "rate_limit_error"}}',
                        {"Content-Type": "application/json"}
                    )
                    
                elif failure_type == 'server_error':
                    flow.response = http.Response.make(
                        500,
                        b'{"error": {"message": "Internal server error"}}',
                        {"Content-Type": "application/json"}
                    )
                    
                elif failure_type == 'network_error':
                    # Simulate network error by closing connection
                    # This will cause a network error in the client
                    flow.kill()
            else:
                print(f"✅ Allowing request #{self.request_count}")

addons = [FlakyLLM()]
