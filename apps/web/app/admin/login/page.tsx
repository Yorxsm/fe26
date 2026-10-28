export default function AdminLogin() {
  return (
    <main className="board">
      <h1>Admin</h1>
      <form method="post" action="/api/admin/login">
        <label>
          Token
          <input name="token" type="password" autoComplete="current-password" required />
        </label>
        <button type="submit" className="btn">
          Enter
        </button>
      </form>
    </main>
  );
}
