import assert from "node:assert/strict";
import { test } from "node:test";

import {
  formatMsat,
  formatSatsAmount,
  formatSatsFromMsat,
  parseSatsInputToMsat,
  truncateFederationId,
} from "./format.ts";

test("formatSatsFromMsat hides a zero fractional part", () => {
  assert.equal(formatSatsFromMsat(123456000), "123,456");
  assert.equal(formatSatsFromMsat(0), "0");
  assert.equal(formatSatsFromMsat(1000), "1");
});

test("formatSatsFromMsat shows sub-sat remainders without trailing zeros", () => {
  assert.equal(formatSatsFromMsat(123456789), "123,456.789");
  assert.equal(formatSatsFromMsat(500), "0.5");
  assert.equal(formatSatsFromMsat(1001), "1.001");
  assert.equal(formatSatsFromMsat(2100), "2.1");
});

test("formatSatsAmount is singular-aware", () => {
  assert.equal(formatSatsAmount(1000), "1 sat");
  assert.equal(formatSatsAmount(2000), "2 sats");
  assert.equal(formatSatsAmount(500), "0.5 sats");
});

test("formatMsat groups thousands", () => {
  assert.equal(formatMsat(123456000), "123,456,000 msat");
  assert.equal(formatMsat(0), "0 msat");
});

test("parseSatsInputToMsat parses whole and fractional sats", () => {
  assert.deepEqual(parseSatsInputToMsat("21"), { ok: true, msat: 21000 });
  assert.deepEqual(parseSatsInputToMsat("0.5"), { ok: true, msat: 500 });
  assert.deepEqual(parseSatsInputToMsat("1.001"), { ok: true, msat: 1001 });
  assert.deepEqual(parseSatsInputToMsat(" 1,234 "), {
    ok: true,
    msat: 1234000,
  });
});

test("parseSatsInputToMsat rejects invalid input", () => {
  assert.equal(parseSatsInputToMsat("").ok, false);
  assert.equal(parseSatsInputToMsat("abc").ok, false);
  assert.equal(parseSatsInputToMsat("-5").ok, false);
  assert.equal(parseSatsInputToMsat("1.2345").ok, false, "finer than 1 msat");
  assert.equal(parseSatsInputToMsat("0").ok, false);
  assert.equal(parseSatsInputToMsat("0.000").ok, false);
  assert.equal(parseSatsInputToMsat("1e3").ok, false);
});

test("truncateFederationId keeps short ids and truncates long ones", () => {
  assert.equal(truncateFederationId("abcdef"), "abcdef");
  const id = "fed11aabbccddeeff00112233445566778899";
  const truncated = truncateFederationId(id);
  assert.equal(truncated, "fed11aabbc…778899");
});
