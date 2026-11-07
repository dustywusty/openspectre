import { spawnSync } from 'node:child_process';

// The complete research fixture can exceed Node's maximum string length.
// Parse it with the research toolchain, returning only the requested routine.
export function readRecoveredCases(routine) {
  const result = spawnSync('python3', ['-c', `
import json, sys
with open('.local/disassembly/recovered-vectors.json') as source:
    data = json.load(source)
cases = data if isinstance(data, list) else data['cases']
json.dump([case for case in cases if case.get('routine') == sys.argv[1]], sys.stdout)
`, routine], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(result.stderr || 'Could not read research fixtures');
  return JSON.parse(result.stdout);
}
