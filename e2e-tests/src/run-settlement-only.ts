import { createSettlementTests } from './tests/settlement-tests.js';

async function run() {
  console.log('\n=== SETTLEMENT TESTS ONLY ===\n');

  const suite = createSettlementTests();
  const result = await suite.run(true);

  console.log('\n=== RESULTS ===');
  console.log(`Passed: ${result.passed}`);
  console.log(`Failed: ${result.failed}`);

  if (result.failed > 0) {
    console.log('\nFailed tests:');
    for (const test of result.tests) {
      if (!test.passed) {
        console.log(`  - ${test.name}: ${test.error}`);
      }
    }
  }

  process.exit(result.failed > 0 ? 1 : 0);
}

run().catch(e => {
  console.error('Test runner error:', e);
  process.exit(1);
});
