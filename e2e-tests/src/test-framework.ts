export interface TestResult {
  name: string;
  passed: boolean;
  durationMs: number;
  error?: string;
  details?: string;
}

export interface TestSuiteResult {
  name: string;
  passed: number;
  failed: number;
  skipped: number;
  durationMs: number;
  tests: TestResult[];
}

export type TestFn = () => Promise<void>;

export class TestSuite {
  private readonly name: string;
  private tests: Array<{ name: string; fn: TestFn; skip?: boolean }> = [];
  private beforeAllFn?: () => Promise<void>;
  private afterAllFn?: () => Promise<void>;
  private beforeEachFn?: () => Promise<void>;
  private afterEachFn?: () => Promise<void>;

  constructor(name: string) {
    this.name = name;
  }

  beforeAll(fn: () => Promise<void>): void {
    this.beforeAllFn = fn;
  }

  afterAll(fn: () => Promise<void>): void {
    this.afterAllFn = fn;
  }

  beforeEach(fn: () => Promise<void>): void {
    this.beforeEachFn = fn;
  }

  afterEach(fn: () => Promise<void>): void {
    this.afterEachFn = fn;
  }

  test(name: string, fn: TestFn): void {
    this.tests.push({ name, fn });
  }

  skip(name: string, fn: TestFn): void {
    this.tests.push({ name, fn, skip: true });
  }

  async run(verbose: boolean = false): Promise<TestSuiteResult> {
    const results: TestResult[] = [];
    const suiteStart = Date.now();

    if (verbose) {
      console.log(`\n${'='.repeat(60)}`);
      console.log(`Suite: ${this.name}`);
      console.log('='.repeat(60));
    }

    try {
      if (this.beforeAllFn) {
        await this.beforeAllFn();
      }

      for (const { name, fn, skip } of this.tests) {
        if (skip) {
          results.push({ name, passed: true, durationMs: 0, details: 'SKIPPED' });
          if (verbose) {
            console.log(`  ○ ${name} (skipped)`);
          }
          continue;
        }

        const testStart = Date.now();

        try {
          if (this.beforeEachFn) {
            await this.beforeEachFn();
          }

          await fn();

          if (this.afterEachFn) {
            await this.afterEachFn();
          }

          const durationMs = Date.now() - testStart;
          results.push({ name, passed: true, durationMs });

          if (verbose) {
            console.log(`  ✓ ${name} (${durationMs}ms)`);
          }
        } catch (e) {
          const durationMs = Date.now() - testStart;
          const error = e instanceof Error ? e.message : String(e);
          results.push({ name, passed: false, durationMs, error });

          if (verbose) {
            console.log(`  ✗ ${name} (${durationMs}ms)`);
            console.log(`    Error: ${error}`);
          }
        }
      }

      if (this.afterAllFn) {
        await this.afterAllFn();
      }
    } catch (e) {
      // beforeAll or afterAll failed
      const error = e instanceof Error ? e.message : String(e);
      if (verbose) {
        console.log(`  Suite setup/teardown failed: ${error}`);
      }
    }

    const passed = results.filter(r => r.passed && r.details !== 'SKIPPED').length;
    const failed = results.filter(r => !r.passed).length;
    const skipped = results.filter(r => r.details === 'SKIPPED').length;

    return {
      name: this.name,
      passed,
      failed,
      skipped,
      durationMs: Date.now() - suiteStart,
      tests: results,
    };
  }
}

export function assert(condition: boolean, message: string): void {
  if (!condition) {
    throw new Error(`Assertion failed: ${message}`);
  }
}

export function assertEqual<T>(actual: T, expected: T, message?: string): void {
  if (actual !== expected) {
    throw new Error(
      message || `Expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`
    );
  }
}

export function assertGreater(actual: number, expected: number, message?: string): void {
  if (actual <= expected) {
    throw new Error(message || `Expected ${actual} > ${expected}`);
  }
}

export function assertLess(actual: number, expected: number, message?: string): void {
  if (actual >= expected) {
    throw new Error(message || `Expected ${actual} < ${expected}`);
  }
}

export function assertInRange(actual: number, min: number, max: number, message?: string): void {
  if (actual < min || actual > max) {
    throw new Error(message || `Expected ${actual} to be in range [${min}, ${max}]`);
  }
}

export function assertNotNull<T>(value: T | null | undefined, message?: string): asserts value is T {
  if (value === null || value === undefined) {
    throw new Error(message || `Expected non-null value`);
  }
}

export async function retry<T>(
  fn: () => Promise<T>,
  maxAttempts: number = 3,
  delayMs: number = 1000
): Promise<T> {
  let lastError: Error | null = null;

  for (let i = 0; i < maxAttempts; i++) {
    try {
      return await fn();
    } catch (e) {
      lastError = e instanceof Error ? e : new Error(String(e));
      if (i < maxAttempts - 1) {
        await new Promise(resolve => setTimeout(resolve, delayMs));
      }
    }
  }

  throw lastError;
}

export async function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}
