import z from "zod";

const FileMetadataSchema = z.object({
  display_name: z.string(),
  file_size: z.number(),
  file_description: z.string(),
});

export const CheckFileExistsSchema = z.object({
  file_name: z.string(),
});

export const FileMetadataArraySchema = z.object({
  files: z.array(FileMetadataSchema),
});

export const PresignedUrlSchema = z.object({
  presigned_url: z.string(),
});

export type FileMetadata = z.infer<typeof FileMetadataSchema>;
export type PresignedUrl = z.infer<typeof PresignedUrlSchema>;
