import { Item, ItemContent, ItemMedia, ItemTitle } from "@/components/ui/item";
import { Spinner } from "@/components/ui/spinner";

type LoadingSpinnerProps = {
  message: string;
};

export function LoadingSpinner(props: LoadingSpinnerProps) {
  return (
    <div className="flex w-full max-w-xs flex-col gap-4 [--radius:1rem]">
      <Item variant="muted">
        <ItemMedia>
          <Spinner />
        </ItemMedia>
        <ItemContent>
          <ItemTitle className="line-clamp-1">{props.message}</ItemTitle>
        </ItemContent>
      </Item>
    </div>
  );
}
