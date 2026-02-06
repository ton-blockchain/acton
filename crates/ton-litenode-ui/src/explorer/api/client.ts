import { FullAccountState, Transaction } from "../types";

export class TonClient {
  private v2BaseUrl: string;
  private v3BaseUrl: string;

  constructor(baseUrl: string) {
    this.v2BaseUrl = baseUrl.endsWith("/v2") ? baseUrl : `${baseUrl}/v2`;
    this.v3BaseUrl = baseUrl.replace(/\/v2$/, "/v3");
    if (this.v3BaseUrl === baseUrl) {
        this.v3BaseUrl = `${baseUrl}/v3`;
    }
  }

  async getAddressInformation(address: string, seqno?: number): Promise<FullAccountState> {
    const url = this.buildUrl(this.v2BaseUrl, "/getAddressInformation");
    url.searchParams.append("address", address);
    if (seqno !== undefined) {
      url.searchParams.append("seqno", seqno.toString());
    }
    const response = await fetch(url.toString());
    const data = await response.json();
    if (!data.ok) throw new Error(data.error || "Failed to fetch address information");
    return data.result;
  }

  async getTransactions(address: string, limit = 20): Promise<Transaction[]> {
    const url = this.buildUrl(this.v2BaseUrl, "/getTransactions");
    url.searchParams.append("address", address);
    url.searchParams.append("limit", limit.toString());
    const response = await fetch(url.toString());
    const data = await response.json();
    if (!data.ok) throw new Error(data.error || "Failed to fetch transactions");
    return data.result;
  }

  async getTraces(hash: string): Promise<any> {
    const url = this.buildUrl(this.v3BaseUrl, "/traces");
    url.searchParams.append("hash", hash);
    const response = await fetch(url.toString());
    const data = await response.json();
    
    if (data.ok === false) throw new Error(data.error || "Failed to fetch traces");
    
    return data.result;
  }

  private buildUrl(base: string, path: string): URL {
    const fullBase = base.startsWith("http")
      ? base
      : `${window.location.origin}${base}`;
    return new URL(`${fullBase}${path}`);
  }
}
