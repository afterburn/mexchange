import { ServiceMonitor } from './service-monitor.js';
import { TestSuiteResult } from './test-framework.js';
import { createOrderbookTests } from './tests/orderbook-tests.js';
import { createOrderFlowTests } from './tests/order-flow-tests.js';
import { createMatchingTests } from './tests/matching-tests.js';
import { createConnectivityTests } from './tests/connectivity-tests.js';
import { createSettlementTests } from './tests/settlement-tests.js';

interface TestRunResult {
  totalPassed: number;
  totalFailed: number;
  totalSkipped: number;
  totalDurationMs: number;
  suites: TestSuiteResult[];
}

async function runAllTests(verbose: boolean = false, quick: boolean = false): Promise<TestRunResult> {
  console.log('\n' + '='.repeat(60));
  console.log('MEXCHANGE END-TO-END INTEGRATION TESTS');
  console.log('='.repeat(60));
  console.log(`Started at: ${new Date().toISOString()}`);
  console.log(`Mode: ${verbose ? 'verbose' : 'normal'}${quick ? ' (quick)' : ''}`);
  console.log('');

  // Check services first
  console.log('Checking service health...');
  const monitor = new ServiceMonitor();

  try {
    const status = await monitor.waitForAllHealthy(30000);
    console.log(monitor.formatStatus(status));
  } catch (e) {
    console.error('Services not healthy, aborting tests');
    const status = await monitor.checkAllServices();
    console.error(monitor.formatStatus(status));
    process.exit(1);
  }

  console.log('\nRunning test suites...\n');

  const suites = [
    createConnectivityTests(),
    createOrderbookTests(),
    createOrderFlowTests(),
  ];

  // Add matching tests only in full mode (they're slower)
  if (!quick) {
    suites.push(createMatchingTests());
    suites.push(createSettlementTests());
  }

  const results: TestSuiteResult[] = [];
  let totalPassed = 0;
  let totalFailed = 0;
  let totalSkipped = 0;

  for (const suite of suites) {
    const result = await suite.run(verbose);
    results.push(result);
    totalPassed += result.passed;
    totalFailed += result.failed;
    totalSkipped += result.skipped;

    // Print suite summary
    const status = result.failed === 0 ? '✓' : '✗';
    console.log(`${status} ${result.name}: ${result.passed} passed, ${result.failed} failed (${result.durationMs}ms)`);

    if (result.failed > 0 && !verbose) {
      // Print failed tests even in non-verbose mode
      for (const test of result.tests) {
        if (!test.passed) {
          console.log(`    ✗ ${test.name}: ${test.error}`);
        }
      }
    }
  }

  const totalDurationMs = results.reduce((sum, r) => sum + r.durationMs, 0);

  // Print summary
  console.log('\n' + '='.repeat(60));
  console.log('TEST SUMMARY');
  console.log('='.repeat(60));
  console.log(`Total:   ${totalPassed + totalFailed + totalSkipped} tests`);
  console.log(`Passed:  ${totalPassed}`);
  console.log(`Failed:  ${totalFailed}`);
  console.log(`Skipped: ${totalSkipped}`);
  console.log(`Time:    ${(totalDurationMs / 1000).toFixed(2)}s`);
  console.log('');

  if (totalFailed > 0) {
    console.log('FAILED TESTS:');
    for (const suite of results) {
      for (const test of suite.tests) {
        if (!test.passed) {
          console.log(`  - ${suite.name} > ${test.name}`);
          console.log(`    Error: ${test.error}`);
        }
      }
    }
  }

  console.log('\n' + (totalFailed === 0 ? '✓ ALL TESTS PASSED' : '✗ SOME TESTS FAILED'));
  console.log(`Finished at: ${new Date().toISOString()}`);

  return {
    totalPassed,
    totalFailed,
    totalSkipped,
    totalDurationMs,
    suites: results,
  };
}

// Parse command line args
const args = process.argv.slice(2);
const verbose = args.includes('--verbose') || args.includes('-v');
const quick = args.includes('--quick') || args.includes('-q');

runAllTests(verbose, quick)
  .then((result) => {
    process.exit(result.totalFailed > 0 ? 1 : 0);
  })
  .catch((e) => {
    console.error('Test runner failed:', e);
    process.exit(1);
  });
