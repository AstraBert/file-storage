import { getSearchClient } from "@/client/search-client";
import { getFileStorageClient } from "@/client/api-client";
import { useState } from "react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { toast, Toaster } from "sonner";
import {
  Trash,
  Copy,
  Check,
  Share2Icon,
  SearchIcon,
  Loader2,
} from "lucide-react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

type SearchResult = {
  fileName: string;
  fileDescription: string;
};

function parseResult(raw: string): SearchResult {
  const [name, ...rest] = raw.split("\\n\\n");
  const fileName = name.slice(1); // exclude double quotes at the beginning
  const desc = rest.join("\\n\\n");
  const fileDescription = desc.slice(0, desc.length - 1); // exclude double quotes at the end
  return { fileName, fileDescription };
}

export default function Search() {
  const [query, setQuery] = useState<string>("");
  const [limit, setLimit] = useState<string>("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [url, setUrl] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const handleSearch = async () => {
    if (!query.trim()) return;
    const client = getSearchClient();
    setLoading(true);
    try {
      const response = await client.search({
        query,
        limit: limit ? parseInt(limit) : null,
      });
      setResults(response.retrieved.map(parseResult));
    } catch (e) {
      toast.error("Search failed", {
        description: `An error occurred during search: ${e}`,
      });
    } finally {
      setLoading(false);
    }
  };

  const handleGetUrl = async (fileName: string) => {
    try {
      const presigned = await getFileStorageClient().getPresignedUrl(fileName);
      setUrl(presigned);
    } catch (e) {
      toast.error("Error generating URL", {
        description: `${e}`,
      });
    }
  };

  const handleDelete = async (fileName: string) => {
    try {
      await getFileStorageClient().deleteFile(fileName);
      setResults((prev) => prev.filter((r) => r.fileName !== fileName));
      toast.success("File deleted", {
        description: `"${fileName}" was successfully deleted.`,
      });
    } catch (e) {
      toast.error("Error deleting file", {
        description: `${e}`,
      });
    }
  };

  const handleCopy = async () => {
    if (!url) return;
    await navigator.clipboard.writeText(url);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="max-w-4xl mx-auto px-6 py-10 space-y-6">
      <div>
        <div className="flex flex-col items-center align-top">
          <h1 className="text-3xl font-bold bg-linear-to-r from-gray-400 to-gray-600 bg-clip-text text-transparent">
            Search Files
          </h1>
          <h2 className="text-xl mb-2">Find files by name or description</h2>
          <Button variant="link">
            <a href="/">
              <span className="text-lg mb-2">Home</span>
            </a>
          </Button>
        </div>
      </div>

      {/* Search bar */}
      <div className="flex gap-2">
        <Input
          placeholder="Search query..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleSearch()}
          className="flex-1"
        />
        <Input
          placeholder="Limit"
          type="number"
          value={limit}
          onChange={(e) => setLimit(e.target.value)}
          className="w-24"
        />
        <Button onClick={handleSearch} disabled={loading || !query.trim()}>
          {loading ? (
            <Loader2 className="w-4 h-4 animate-spin" />
          ) : (
            <SearchIcon className="w-4 h-4" />
          )}
          Search
        </Button>
      </div>

      {/* Results */}
      {results.length > 0 && (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>File Name</TableHead>
              <TableHead>Description</TableHead>
              <TableHead className="text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {results.map((result) => (
              <TableRow key={result.fileName}>
                <TableCell className="font-medium whitespace-nowrap">
                  {result.fileName}
                </TableCell>
                <TableCell className="text-muted-foreground">
                  {result.fileDescription}
                </TableCell>
                <TableCell className="flex justify-end gap-2">
                  <Button
                    variant="outline"
                    size="icon"
                    className="cursor-pointer"
                    onClick={() => handleGetUrl(result.fileName)}
                  >
                    <Share2Icon className="w-4 h-4" />
                  </Button>
                  <Button
                    variant="outline"
                    size="icon"
                    className="cursor-pointer"
                    onClick={() => handleDelete(result.fileName)}
                  >
                    <Trash className="w-4 h-4 text-red-400" />
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}

      {/* Share URL dialog */}
      <Dialog open={!!url} onOpenChange={(open) => !open && setUrl(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Share Link</DialogTitle>
          </DialogHeader>
          <div className="flex gap-2 items-center">
            <Input value={url ?? ""} readOnly className="font-mono text-sm" />
            <Button variant="outline" size="icon" onClick={handleCopy}>
              {copied ? (
                <Check className="w-4 h-4 text-green-500" />
              ) : (
                <Copy className="w-4 h-4" />
              )}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      <Toaster />
    </div>
  );
}
