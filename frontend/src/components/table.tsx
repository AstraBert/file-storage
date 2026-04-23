import type { FileMetadata } from "@/client/schemas";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { getFileStorageClient } from "@/client/api-client";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Trash, Copy, Check, Share2Icon } from "lucide-react";
import { toast, Toaster } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import type { QueryClient } from "@tanstack/react-query";

export type FilesTableProps = {
  files: FileMetadata[];
  queryClient: QueryClient;
};

function formatFileSize(bytes: number): string {
  if (bytes === 0) return "0 B";

  const units = ["B", "kB", "MB", "GB", "TB"];
  const exponent = Math.min(
    Math.floor(Math.log10(bytes) / 3),
    units.length - 1,
  );
  const value = bytes / Math.pow(1000, exponent);

  return `${parseFloat(value.toFixed(1))} ${units[exponent]}`;
}

export function FilesTable(props: FilesTableProps) {
  const client = getFileStorageClient();
  const [url, setUrl] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const handleGetUrl = async (displayName: string) => {
    try {
      const presigned = await client.getPresignedUrl(displayName);
      setUrl(presigned);
    } catch (e) {
      toast("Error generating URL", {
        description: `An error occurred while generating the presigned url: ${e}`,
      });
    }
  };

  const handleDelete = async (displayName: string) => {
    try {
      await client.deleteFile(displayName);
      await props.queryClient.invalidateQueries({ queryKey: ["files"] });
      toast("File deleted", {
        description: `"${displayName}" was successfully deleted.`,
      });
    } catch (e) {
      toast("Error deleting file", {
        description: `An error occurred while deleting the file: ${e}`,
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
    <>
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="w-25">File Name</TableHead>
            <TableHead>Size</TableHead>
            <TableHead className="text-right">Description</TableHead>
            <TableHead>Actions</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {props.files.map((file) => (
            <TableRow key={file.display_name}>
              <TableCell className="font-medium">{file.display_name}</TableCell>
              <TableCell>{formatFileSize(file.file_size)}</TableCell>
              <TableCell className="text-right">
                {file.file_description}
              </TableCell>
              <TableCell className="flex gap-2">
                <Button
                  variant="outline"
                  className="cursor-pointer"
                  onClick={() => handleGetUrl(file.display_name)}
                >
                  <Share2Icon />
                </Button>
                <Button
                  variant="outline"
                  className="cursor-pointer"
                  onClick={() => handleDelete(file.display_name)}
                >
                  <Trash className="text-red-400" />
                </Button>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>

      <Dialog open={!!url} onOpenChange={(open) => !open && setUrl(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Share Link</DialogTitle>
          </DialogHeader>
          <div className="flex gap-2 items-center">
            <Input value={url ?? ""} readOnly className="font-mono text-sm" />
            <Button variant="outline" size="icon" onClick={handleCopy}>
              {copied ? <Check className="text-green-500" /> : <Copy />}
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      <Toaster />
    </>
  );
}
