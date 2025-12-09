import { createFullFlowTests } from './tests/full-flow-tests.js';

async function run() {
  console.log('\n=== FULL FLOW E2E TESTS ===');
  console.log('These tests verify database state at each step\n');

  const suite = createFullFlowTests();
  const result = await suite.run(true);

  console.log('\n=== RESULTS ===');
  console.log(`Passed: ${result.passed}`);
  console.log(`Failed: ${result.failed}`);
  console.log(`Duration: ${result.durationMs}ms`);

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
