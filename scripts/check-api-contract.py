#!/usr/bin/env python3
"""Checks that the Dogechain.com OpenAPI spec still guarantees every field the
CLI requires: present (in `required`) and never null.

The list below mirrors the non-Option fields of src/model.rs; keep the two in
step. Usage: check-api-contract.py [spec.json]  (default: the live spec)
"""

import json
import sys
import urllib.request

SPEC_URL = "https://dogechain.com/openapi.json"

# (endpoint, path to an object inside `data`, fields the CLI requires)
REQUIRED = [
    ("/block/{id}", [], ["height", "hash", "time", "num_txs", "size", "confirmations",
                         "reward", "fees", "value_out", "difficulty"]),
    ("/blocks/latest", [], ["blocks"]),
    ("/blocks/latest", ["blocks", "[]"], ["height", "time", "num_txs", "reward_and_fees"]),
    ("/tx/{txid}", [], ["transaction", "explain"]),
    ("/tx/{txid}", ["transaction"], ["hash", "confirmations", "time", "fee", "size",
                                     "inputs", "outputs"]),
    ("/tx/{txid}", ["transaction", "outputs", "[]"], ["index", "value"]),
    ("/tx/{txid}", ["explain"], ["headline", "detail"]),
    ("/tx/{txid}", ["warnings", "[]"], ["kind"]),
    ("/address/{address}", [], ["address", "page", "summary", "transactions"]),
    ("/address/{address}", ["summary"], ["confirmed_balance", "unconfirmed_balance",
                                         "confirmed_received", "txs_total"]),
    ("/address/{address}", ["transactions", "[]"], ["hash", "time", "balance_change"]),
    ("/labels", [], ["labels"]),
    ("/network", [], ["info"]),
    ("/network", ["info"], ["block_count", "best_block_hash", "hashrate", "mempool"]),
    ("/network", ["info", "mempool"], ["mempool_txs", "mempool_size"]),
    ("/network", ["info", "mempool", "blocks", "[]"], ["num_txs", "median_fee_rate"]),
    ("/fees", [], ["fee_rate_koinu_per_byte", "simple_payment", "mempool"]),
    ("/fees", ["fee_rate_koinu_per_byte"], ["min"]),
    ("/fees", ["simple_payment"], ["bytes", "min_fee"]),
    ("/fees", ["mempool"], ["txs", "bytes"]),
    ("/supply", [], ["supply", "height", "per_block", "per_year", "inflation_next_12_months"]),
    ("/richlist", [], ["height", "supply", "page", "total_rows", "rows"]),
    ("/richlist", ["rows", "[]"], ["rank", "balance", "share_of_supply"]),
    ("/chart/{series}", [], ["series", "interval", "points"]),
    ("/chart/{series}", ["points", "[]"], ["t"]),
    ("/find", [], ["kind"]),
]


def main():
    if len(sys.argv) > 1:
        with open(sys.argv[1]) as f:
            spec = json.load(f)
    else:
        request = urllib.request.Request(
            SPEC_URL, headers={"User-Agent": "dogechain-cli-contract-check"}
        )
        with urllib.request.urlopen(request, timeout=30) as r:
            spec = json.load(r)

    def res(s):
        while isinstance(s, dict) and "$ref" in s:
            node = spec
            for key in s["$ref"].lstrip("#/").split("/"):
                node = node[key]
            s = node
        return s

    def merge(s):
        s = res(s)
        if "allOf" in s:
            m = {"properties": {}, "required": []}
            for part in s["allOf"]:
                part = merge(part)
                m["properties"].update(part.get("properties", {}))
                m["required"] += part.get("required", [])
            return m
        return s

    def non_null(s):
        s = res(s)
        for key in ("anyOf", "oneOf"):
            if key in s:
                opts = [res(x) for x in s[key] if res(x).get("type") != "null"]
                if len(opts) == 1:
                    return opts[0]
        return s

    def nullable(s):
        s = res(s)
        t = s.get("type")
        if isinstance(t, list) and "null" in t:
            return True
        return any(res(x).get("type") == "null"
                   for key in ("anyOf", "oneOf") for x in s.get(key, []))

    def data_of(endpoint):
        op = spec["paths"][endpoint]["get"]
        body = merge(op["responses"]["200"]["content"]["application/json"]["schema"])
        return res(body["properties"]["data"])

    problems = []
    for endpoint, path, fields in REQUIRED:
        where = f"{endpoint} data" + "".join(f".{p}" for p in path)
        try:
            obj = data_of(endpoint)
            for part in path:
                obj = merge(non_null(obj))
                obj = res(obj["items"]) if part == "[]" else res(obj["properties"][part])
            obj = merge(non_null(obj))
        except KeyError as e:
            problems.append(f"{where}: missing from the spec ({e})")
            continue
        required = set(obj.get("required", []))
        props = obj.get("properties", {})
        for field in fields:
            if field not in props:
                problems.append(f"{where}.{field}: missing from the spec")
            elif field not in required:
                problems.append(f"{where}.{field}: no longer required")
            elif nullable(props[field]):
                problems.append(f"{where}.{field}: can now be null")

    for p in problems:
        print(f"::error::{p}")
    checked = sum(len(f) for _, _, f in REQUIRED)
    print(f"{checked} fields checked, {len(problems)} problems")
    sys.exit(1 if problems else 0)


if __name__ == "__main__":
    main()
