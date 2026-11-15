<script lang="ts" module>
	import { type VariantProps, tv } from "tailwind-variants";

	export const badgeVariants = tv({
		base: "h-6 gap-1 rounded-full border px-2.5 text-xs font-medium has-data-[icon=inline-end]:pr-1.5 has-data-[icon=inline-start]:pl-1.5 [&>svg]:size-3! group/badge inline-flex w-fit shrink-0 items-center justify-center overflow-hidden whitespace-nowrap focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring aria-invalid:border-destructive aria-invalid:ring-destructive/20 [&>svg]:pointer-events-none",
		// Insignias de estado: texto del color de la ficha sobre la superficie, con borde del mismo tono (el contraste del texto es el medido).
		variants: {
			variant: {
				default: "text-primary border-[color-mix(in_srgb,var(--primary)_45%,transparent)]",
				secondary: "text-muted-foreground border-border",
				success: "text-success border-[color-mix(in_srgb,var(--success)_45%,transparent)]",
				warning: "text-warning border-[color-mix(in_srgb,var(--warning)_45%,transparent)]",
				destructive: "text-destructive border-[color-mix(in_srgb,var(--danger)_45%,transparent)]",
				outline: "border-input text-foreground",
			},
		},
		defaultVariants: {
			variant: "default",
		},
	});

	export type BadgeVariant = VariantProps<typeof badgeVariants>["variant"];
</script>

<script lang="ts">
	import { cn, type WithElementRef } from "$lib/utils.js";
	import type { HTMLAnchorAttributes } from "svelte/elements";

	let {
		ref = $bindable(null),
		href,
		class: className,
		variant = "default",
		children,
		...restProps
	}: WithElementRef<HTMLAnchorAttributes> & {
		variant?: BadgeVariant;
	} = $props();
</script>

<svelte:element
	this={href ? "a" : "span"}
	bind:this={ref}
	data-slot="badge"
	{href}
	class={cn(badgeVariants({ variant }), className)}
	{...restProps}
>
	{@render children?.()}
</svelte:element>
