import type { FileMetadata } from "@/client/schemas";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

export type FilesTableProps = {
  files: FileMetadata[];
};

export function FilesTable(props: FilesTableProps) {
  return (
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead className="w-25">File Name</TableHead>
          <TableHead>Size</TableHead>
          <TableHead className="text-right">Description</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {props.files.map((file) => (
          <TableRow key={file.display_name}>
            <TableCell className="font-medium">{file.display_name}</TableCell>
            <TableCell>{file.file_size}</TableCell>
            <TableCell className="text-right">
              {file.file_description}
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
