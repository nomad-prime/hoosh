"""
mitmproxy addon for simulating flaky LLM APIs
Usage: mitmproxy -s flaky_api.py --set failure_rate=0.3 --set failure_types=rate_limit,server_error
and run hoosh with  export HTTPS_PROXY=http://localhost:8080 && export HTTP_PROXY=http://localhost:8080 && cargo run
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
        loader.add_option(
            name="failure_types",
            typespec=str,
            default="rate_limit,server_error,network_error,auth_error,invalid_request",
            help="Comma-separated list of failure types to inject (rate_limit, server_error, network_error, auth_error, invalid_request)",
        )

    def request(self, flow: http.HTTPFlow) -> None:
        # Only intercept LLM API calls
        if "together.xyz" in flow.request.host or "anthropic.com" in flow.request.host or "openrouter.ai" in flow.request.host:
            self.request_count += 1

            # Get failure rate from options and convert to float
            failure_rate = float(ctx.options.failure_rate)
            if random.random() < failure_rate:
                print(f"🔴 Injecting failure for request #{self.request_count}")

                # Get configured failure types and randomly choose one
                failure_types = [ft.strip() for ft in ctx.options.failure_types.split(',')]
                failure_type = random.choice(failure_types)
                print(failure_type)
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

                elif failure_type == 'auth_error':
                    flow.response = http.Response.make(
                        401,
                        b'{"error": {"message": "Invalid API key", "type": "authentication_error"}}',
                        {"Content-Type": "application/json"}
                    )

                elif failure_type == 'invalid_request':
                    flow.response = http.Response.make(
                        400,
                        b'{"error": {"message": "Invalid request: missing required parameter", "type": "invalid_request_error"}}',
                        {"Content-Type": "application/json"}
                    )
            else:
                print(f"✅ Allowing request #{self.request_count}")

addons = [FlakyLLM()]
