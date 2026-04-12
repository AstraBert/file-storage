import z from "zod";

const FileMetadataSchema = z.object({
  display_name: z.string(),
  file_size: z.number(),
  file_description: z.string(),
});

export const FileMetadataArraySchema = z.object({
  files: z.array(FileMetadataSchema),
});

export type FileMetadata = z.infer<typeof FileMetadataSchema>;
