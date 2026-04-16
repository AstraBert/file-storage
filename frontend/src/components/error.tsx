import { CircleX } from "lucide-react";

import {
  Item,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item";

type ErrorBannerProps = {
  message: string;
};

export function ErrorBanner(props: ErrorBannerProps) {
  return (
    <div className="flex w-full max-w-lg flex-col gap-6 bg-red-300">
      <Item variant="outline">
        <ItemMedia variant="icon">
          <CircleX />
        </ItemMedia>
        <ItemContent>
          <ItemTitle>An error occurred</ItemTitle>
          <ItemDescription>{props.message}</ItemDescription>
        </ItemContent>
      </Item>
    </div>
  );
}
