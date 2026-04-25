import keycloak from "../auth/keycloak";
import type { SearchRequest, SearchResponse } from "./schemas";
import { SearchResponseSchema } from "./schemas";

class SearchClient {
  readonly baseUrl: string;

  constructor() {
    this.baseUrl = "/qdrant";
  }

  private async getToken() {
    let token = "";
    if (keycloak.token) {
      // Refresh token if needed
      await keycloak.updateToken(30); // Refresh if expires in 30s
      token = keycloak.token;
    }
    return token;
  }

  async search({
    query,
    limit = null,
  }: {
    query: string;
    limit?: number | null;
  }): Promise<SearchResponse> {
    const token = await this.getToken();
    const searchInput: SearchRequest = { query, limit };
    const response = await fetch(`${this.baseUrl}/search`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(searchInput),
    });
    if (!response.ok) {
      const details = await response.text();
      throw new Error(
        `Response returned with status ${response.status}: ${details}`,
      );
    }
    const jsonResponse = await response.json();
    const validated = await SearchResponseSchema.parseAsync(jsonResponse);
    return validated;
  }
}

const searchClient = new SearchClient();

export function getSearchClient() {
  return searchClient;
}
