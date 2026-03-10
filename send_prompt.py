import json
import sys
import urllib.request

OLLAMA_URL = "http://192.168.1.178:11434"
MODEL = "qwen3:32b"
NUM_CTX = 98304
TIMEOUT = 600

system_prompt = open("prompt.md").read()
fundamentals = open("fundamentals.json").read()

# Token estimate sanity check
total_chars = len(system_prompt) + len(fundamentals)
estimated_tokens = total_chars // 4
print(f"[info] system prompt: {len(system_prompt):,} chars", flush=True)
print(f"[info] fundamentals:  {len(fundamentals):,} chars", flush=True)
print(f"[info] estimated tokens: ~{estimated_tokens:,} (num_ctx: {NUM_CTX:,})", flush=True)

if estimated_tokens > NUM_CTX:
    print(f"[warn] estimated tokens exceed num_ctx — consider increasing NUM_CTX", flush=True)

print(f"[info] sending request to {OLLAMA_URL} ...\n", flush=True)

payload = json.dumps({
    "model": MODEL,
    "stream": True,
    "options": {
        "num_ctx": NUM_CTX,
    },
    "messages": [
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": fundamentals},
    ]
}).encode()

req = urllib.request.Request(
    f"{OLLAMA_URL}/api/chat",
    data=payload,
    headers={"Content-Type": "application/json"},
    method="POST"
)

try:
    with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
        print(f"[info] HTTP {resp.status} — streaming response:\n", flush=True)
        for line in resp:
            if not line.strip():
                continue
            chunk = json.loads(line)
            if "error" in chunk:
                print(f"\n[error] {chunk['error']}", flush=True)
                sys.exit(1)
            content = chunk.get("message", {}).get("content", "")
            print(content, end="", flush=True)
            if chunk.get("done"):
                stats = {
                    "eval_count": chunk.get("eval_count"),
                    "eval_duration_s": round(chunk.get("eval_duration", 0) / 1e9, 1),
                    "total_duration_s": round(chunk.get("total_duration", 0) / 1e9, 1),
                    "tokens_per_sec": round(
                        chunk.get("eval_count", 0) /
                        max(chunk.get("eval_duration", 1) / 1e9, 0.001), 1
                    ),
                }
                print(f"\n\n[done] {stats}", flush=True)

except urllib.error.URLError as e:
    print(f"\n[error] connection failed: {e}", flush=True)
    sys.exit(1)
except TimeoutError:
    print(f"\n[error] request timed out after {TIMEOUT}s", flush=True)
    sys.exit(1)
