import { ThemeToggle } from "@/components/theme-toggle";

function App() {
  return (
    <div className="flex h-screen flex-col">
      <header className="flex h-12 items-center justify-between border-b px-4">
        <span className="font-semibold">Orbis Scriptor</span>
        <ThemeToggle />
      </header>
      <main className="flex flex-1 items-center justify-center text-muted-foreground">
        Nothing here yet.
      </main>
    </div>
  );
}

export default App;
