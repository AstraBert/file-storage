import { CircleX } from "lucide-react";

import {
  Item,
  ItemContent,
  ItemDescription,
  ItemMedia,
  ItemTitle,
} from "@/components/ui/item";

type BannerProps = {
  message: string;
  error: boolean;
};

export function Banner(props: BannerProps) {
  const bg = props.error ? "bg-red-300" : "bg-green-300";
  const title = props.error ? "An error occurred" : "Success!";
  return (
    <div className={`flex w-full max-w-lg flex-col gap-6 ${bg}`}>
      <Item variant="outline">
        <ItemMedia variant="icon">
          <CircleX />
        </ItemMedia>
        <ItemContent>
          <ItemTitle>{title}</ItemTitle>
          <ItemDescription>{props.message}</ItemDescription>
        </ItemContent>
      </Item>
    </div>
  );
}
