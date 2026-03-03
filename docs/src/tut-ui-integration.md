# Tutorial: UI Integration

Learn how to build modern, reactive user interfaces with Vox. This tutorial covers the `@component` decorator, JSX-like syntax, and binding UI state to backend logic.

## 1. The `@component` Decorator

Vox UI components are defined with the `@component` decorator. They look and feel like React components but are compiled for maximum performance.

```vox
# Skip-Test
@component fn Profile(name: str, bio: str) to Element:
    <div class="p-6 bg-white shadow rounded-lg">
        <h2 class="text-xl font-bold">name</h2>
        <p class="text-gray-600">bio</p>
    </div>
```

## 2. JSX in Vox

Vox supports a JSX-like syntax directly in `.vox` files. You can embed variables, use loops, and conditionally render elements.

```vox
# Skip-Test
@component fn UserList(users: list[str]) to Element:
    <ul class="divide-y">
        for user in users:
            <li class="py-2">user</li>
    </ul>
```

## 3. Styling with Scoped Blocks

Components can include scoped style blocks. These styles are automatically hashed to prevent global namespace pollution.

```vox
# Skip-Test
@component fn FancyButton(label: str) to Element:
    <button class="my-btn">label</button>

    style:
        .my-btn {
            background: linear-gradient(to right, #4f46e5, #06b6d4);
            color: white;
            padding: 0.5rem 1rem;
            border-radius: 9999px;
            transition: transform 0.2s;
        }
        .my-btn:hover {
            transform: scale(1.05);
        }
```

## 4. Binding to Backend Logic

The true power of Vox lies in its technical unification. You can call `@server` functions or interact with actors directly from your UI event handlers.

```vox
# Skip-Test
@server fn subscribe(email: str) to bool:
    ret true

@component fn NewsletterForm() to Element:
    <form on_submit={e => subscribe(e.target.email.value)}>
        <input type="email" name="email" />
        <button type="submit">"Join"</button>
    </form>
```

## 5. Summary

Vox UI integration provides:
- **Type Safety**: Backend types flow seamlessly into the frontend.
- **Developer Ergonomics**: Write full-stack logic in a single language.
- **High Performance**: Compiled UI with minimal runtime overhead.

---

**Next Steps**:
- [Language Reference](ref-language.md) — Detailed JSX specification.
- [First App](tut-first-app.md) — Apply these UI patterns to your todo list.
