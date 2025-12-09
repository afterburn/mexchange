export interface ServiceStatus {
  name: string;
  healthy: boolean;
  latencyMs?: number;
  error?: string;
}

export interface SystemStatus {
  allHealthy: boolean;
  services: ServiceStatus[];
  timestamp: Date;
}

export class ServiceMonitor {
  private readonly gatewayUrl: string;
  private readonly matchingEngineUrl: string;
  private readonly accountsUrl: string;

  constructor(
    gatewayUrl: string = 'http://localhost:3000',
    matchingEngineUrl: string = 'http://localhost:8080',
    accountsUrl: string = 'http://localhost:3001'
  ) {
    this.gatewayUrl = gatewayUrl;
    this.matchingEngineUrl = matchingEngineUrl;
    this.accountsUrl = accountsUrl;
  }

  async checkService(name: string, url: string): Promise<ServiceStatus> {
    const start = Date.now();
    try {
      const response = await fetch(`${url}/health`, {
        signal: AbortSignal.timeout(5000),
      });
      const latencyMs = Date.now() - start;

      if (response.ok) {
        return { name, healthy: true, latencyMs };
      } else {
        return {
          name,
          healthy: false,
          latencyMs,
          error: `HTTP ${response.status}`,
        };
      }
    } catch (e) {
      return {
        name,
        healthy: false,
        latencyMs: Date.now() - start,
        error: e instanceof Error ? e.message : String(e),
      };
    }
  }

  async checkAllServices(): Promise<SystemStatus> {
    const [gateway, matchingEngine, accounts] = await Promise.all([
      this.checkService('gateway', this.gatewayUrl),
      this.checkService('matching-engine', this.matchingEngineUrl),
      this.checkService('accounts', this.accountsUrl),
    ]);

    const services = [gateway, matchingEngine, accounts];
    const allHealthy = services.every(s => s.healthy);

    return {
      allHealthy,
      services,
      timestamp: new Date(),
    };
  }

  async waitForAllHealthy(timeoutMs: number = 30000, intervalMs: number = 1000): Promise<SystemStatus> {
    const deadline = Date.now() + timeoutMs;

    while (Date.now() < deadline) {
      const status = await this.checkAllServices();
      if (status.allHealthy) {
        return status;
      }

      await new Promise(resolve => setTimeout(resolve, intervalMs));
    }

    const finalStatus = await this.checkAllServices();
    if (!finalStatus.allHealthy) {
      const unhealthy = finalStatus.services
        .filter(s => !s.healthy)
        .map(s => `${s.name}: ${s.error}`)
        .join(', ');
      throw new Error(`Services not healthy after ${timeoutMs}ms: ${unhealthy}`);
    }

    return finalStatus;
  }

  formatStatus(status: SystemStatus): string {
    const lines = [
      `System Status: ${status.allHealthy ? '✓ ALL HEALTHY' : '✗ UNHEALTHY'}`,
      `Timestamp: ${status.timestamp.toISOString()}`,
      '',
    ];

    for (const service of status.services) {
      const icon = service.healthy ? '✓' : '✗';
      const latency = service.latencyMs ? `${service.latencyMs}ms` : 'N/A';
      const error = service.error ? ` (${service.error})` : '';
      lines.push(`  ${icon} ${service.name}: ${latency}${error}`);
    }

    return lines.join('\n');
  }
}
