<script lang="ts">
    import * as Carousel from "$lib/components/ui/carousel";
    import * as Card from "$lib/components/ui/card";
    import { Button } from "$lib/components/ui/button";
    import { invoke } from "@tauri-apps/api/core";

    async function setTheme(theme: string) {
        await invoke("set_theme", { theme });
        localStorage.setItem("theme", theme);
        if (theme === "dark") {
            document.documentElement.classList.add("dark");
        } else {
            document.documentElement.classList.remove("dark");
        }
    }

    async function finishOnboarding() {
        await invoke("close_current_window");
    }
</script>

<main
    class="min-h-screen bg-background text-foreground flex items-center justify-center"
>
    <Carousel.Root class="w-[80vw]">
        <Carousel.Content>
            <Carousel.Item>
                <Card.Root>
                    <Card.Header>
                        <Card.Title>Welcome to Nuko!</Card.Title>
                        <Card.Description>
                            <span class="italic">what is "Nuko" anyway?</span>
                        </Card.Description>
                    </Card.Header>
                    <Card.Content>
                        <p>
                            Nuko (a cute mispelling of "neko", meaning cat in
                            Japanese) is a powerful tool for managing your game
                            instances, allowing you to easily create, manage,
                            and launch your game instances with just a few
                            clicks.
                            <br />
                            <br />
                            Now, let's get you to customize Nuko to your liking!
                        </p>
                    </Card.Content>
                </Card.Root>
            </Carousel.Item>
            <Carousel.Item>
                <Card.Root>
                    <Card.Header>
                        <Card.Title>Dark theme or light theme?</Card.Title>
                        <Card.Description>
                            <span class="italic"
                                >there is only one right answer.<span>
                                </span></span
                            ></Card.Description
                        >
                    </Card.Header>
                    <Card.Content class="flex gap-4 flex-col">
                        <Button
                            onclick={() => setTheme("light")}
                            variant="outline"
                            class="w-full cursor-pointer">Light</Button
                        >
                        <Button
                            onclick={() => setTheme("dark")}
                            variant="default"
                            class="w-full cursor-pointer">Dark</Button
                        >
                    </Card.Content>
                </Card.Root>
            </Carousel.Item>
            <Carousel.Item>
                <Card.Root>
                    <Card.Header>
                        <Card.Title>All set!</Card.Title>
                        <Card.Description>
                            you're good to go! yippie!
                        </Card.Description>
                    </Card.Header>
                    <Card.Content>
                        <Button
                            onclick={finishOnboarding}
                            class="w-full cursor-pointer">Get Started</Button
                        >
                    </Card.Content>
                </Card.Root>
            </Carousel.Item>
        </Carousel.Content>
        <Carousel.Previous />
        <Carousel.Next />
    </Carousel.Root>
</main>
