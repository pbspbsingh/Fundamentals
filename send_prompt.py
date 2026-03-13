import json
import os
import sys
import urllib.request

DEEPSEEK_URL = "https://api.deepseek.com"
MODEL = "deepseek-reasoner"
TIMEOUT = 600

system_prompt = open("prompt.md").read()
fundamentals = open("fundamentals.json").read()

# Token estimate sanity check
total_chars = len(system_prompt) + len(fundamentals)
estimated_tokens = total_chars // 4
print(f"[info] system prompt: {len(system_prompt):,} chars", flush=True)
print(f"[info] fundamentals:  {len(fundamentals):,} chars", flush=True)
print(f"[info] estimated tokens: ~{estimated_tokens:,}", flush=True)

print(f"[info] sending request to {DEEPSEEK_URL} ...\n", flush=True)

api_key = "<< Your API KEY here >>"

payload = json.dumps({
    "model": MODEL,
    "stream": True,
    "messages": [
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": fundamentals},
    ]
}).encode()

req = urllib.request.Request(
    f"{DEEPSEEK_URL}/chat/completions",
    data=payload,
    headers={
        "Content-Type": "application/json",
        "Authorization": f"Bearer {api_key}",
    },
    method="POST"
)

try:
    with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
        print(f"[info] HTTP {resp.status} — streaming response:\n", flush=True)
        prompt_tokens = 0
        completion_tokens = 0
        for line in resp:
            line = line.strip()
            if not line.startswith(b"data: "):
                continue
            line = line[6:]
            if line == b"[DONE]":
                continue
            chunk = json.loads(line)
            if "error" in chunk:
                print(f"\n[error] {chunk['error']}", flush=True)
                sys.exit(1)
            delta = chunk.get("choices", [{}])[0].get("delta", {})
            content = delta.get("content", "")
            if content:
                print(content, end="", flush=True)
            # Capture usage from the final chunk
            if chunk.get("usage"):
                prompt_tokens = chunk["usage"].get("prompt_tokens", 0)
                completion_tokens = chunk["usage"].get("completion_tokens", 0)
            if chunk.get("choices", [{}])[0].get("finish_reason") == "stop":
                stats = {
                    "prompt_tokens": prompt_tokens,
                    "completion_tokens": completion_tokens,
                    "total_tokens": prompt_tokens + completion_tokens,
                }
                print(f"\n\n[done] {stats}", flush=True)

except urllib.error.URLError as e:
    print(f"\n[error] connection failed: {e}", flush=True)
    sys.exit(1)
except TimeoutError:
    print(f"\n[error] request timed out after {TIMEOUT}s", flush=True)
    sys.exit(1)