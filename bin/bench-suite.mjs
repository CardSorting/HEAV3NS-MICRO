#!/usr/bin/env node
/** [LAYER: UI] */
/**
 * Automated CLI runner for HEAV3NS-MICRO comprehensive benchmarking and stress testing.
 * Evaluates substrate micro-benchmarks, concurrency stress dynamics, and real-world multi-language
 * workspace tasks across /Users/bozoegg/Desktop/tests.
 */

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const args = process.argv.slice(2);
const wantJson = args.includes("--json");
const testsDir = args.find(a => !a.startsWith("-")) || "/Users/bozoegg/Desktop/tests";

const heav3nsBin = process.env.HEAV3NS_BIN || path.join(process.env.HOME || "", ".cargo/bin/heav3ns-micro");

if (!fs.existsSync(heav3nsBin)) {
  console.error(`Error: heav3ns-micro binary not found at ${heav3nsBin}`);
  process.exit(1);
}

const proc = spawnSync(heav3nsBin, ["bench", "--stress", "--tests-dir", testsDir], {
  encoding: "utf8",
  maxBuffer: 10 * 1024 * 1024
});

if (proc.status !== 0) {
  console.error("Benchmarking failed:", proc.stderr || proc.stdout);
  process.exit(proc.status || 1);
}

let result;
try {
  result = JSON.parse(proc.stdout);
} catch (e) {
  console.error("Failed to parse benchmark JSON output:", e.message);
  console.log(proc.stdout);
  process.exit(1);
}

if (wantJson) {
  console.log(JSON.stringify(result, null, 2));
  process.exit(0);
}

console.log("\n" + "=".repeat(78));
console.log("  HEAV3NS-MICRO SOVEREIGN BENCHMARKING & STRESS REPORT");
console.log("=".repeat(78));
console.log(`Runtime         : ${result.runtime || "heav3ns-micro"}`);
console.log(`Flight Suite    : ${result.flightSuite?.passed || 0}/${result.flightSuite?.total || 0} passed (${result.flightSuite?.durationMs || 0}ms)`);
console.log(`Workspace Tasks : ${result.workspaceTasks?.passed || 0}/${result.workspaceTasks?.total || 0} scenarios passed (Rate: ${((result.workspaceTasks?.passRate || 0) * 100).toFixed(0)}%)`);
console.log(`Total Wall-Clock: ${result.totalDurationMs || 0}ms\n`);

console.log("-".repeat(78));
console.log("  SUBSTRATE STRESS BENCHMARKS & LATENCY PROFILE");
console.log("-".repeat(78));

const stress = result.stressBenchmarks || {};
const stressTable = [
  { name: "Arena Bump Allocations", count: stress.arenaAllocationStress?.totalAllocations, ops: stress.arenaAllocationStress?.metrics?.throughputOpsPerSec, p50: stress.arenaAllocationStress?.metrics?.p50LatencyUs },
  { name: "Tool Parser Malformed Repairs", count: stress.toolParserRepairStress?.totalRepairs, ops: stress.toolParserRepairStress?.metrics?.throughputOpsPerSec, p50: stress.toolParserRepairStress?.metrics?.p50LatencyUs },
  { name: "Secret Redactor Threat Scans", count: stress.secretRedactorStress?.totalScans, ops: stress.secretRedactorStress?.metrics?.throughputOpsPerSec, p50: stress.secretRedactorStress?.metrics?.p50LatencyUs },
  { name: "Spatial GUI Action Groundings", count: stress.spatialGroundingStress?.totalGroundings, ops: stress.spatialGroundingStress?.metrics?.throughputOpsPerSec, p50: stress.spatialGroundingStress?.metrics?.p50LatencyUs },
  { name: "Session Compaction Passes", count: stress.sessionCompactionStress?.totalCompactions, ops: stress.sessionCompactionStress?.metrics?.throughputOpsPerSec, p50: stress.sessionCompactionStress?.metrics?.p50LatencyUs },
  { name: "Wave Scheduler DAG Partitions", count: stress.waveSchedulerPartitionStress?.totalPartitions, ops: stress.waveSchedulerPartitionStress?.metrics?.throughputOpsPerSec, p50: stress.waveSchedulerPartitionStress?.metrics?.p50LatencyUs },
];

for (const row of stressTable) {
  if (row.count !== undefined) {
    console.log(`  ${row.name.padEnd(32)} | ${(row.count + " ops").padStart(12)} | ${(row.ops + " ops/sec").padStart(18)} | p50: ${row.p50}us`);
  }
}

console.log("\n" + "-".repeat(78));
console.log("  REAL-WORLD MULTI-LANGUAGE WORKSPACE SCENARIOS (/Users/bozoegg/Desktop/tests)");
console.log("-".repeat(78));

const tasks = result.workspaceTasks?.scenarios || [];
for (const task of tasks) {
  const statusStr = task.passed ? "[PASS]" : "[FAIL]";
  console.log(`  ${statusStr} [${task.project}] ${task.name} (${task.durationMs}ms)`);
  for (const step of task.steps || []) {
    console.log(`         • ${step}`);
  }
}

console.log("=".repeat(78) + "\n");
process.exit(result.workspaceTasks?.passRate === 1.0 ? 0 : 1);

