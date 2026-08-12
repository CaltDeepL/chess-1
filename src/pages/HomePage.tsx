import { Link } from "react-router-dom";

export default function HomePage() {
  return (
    <div className="home-page">
      <h1 className="home-title">Glass Chess</h1>
      <p className="home-subtitle">
        ガラス盤の上で、リアルタイムにオンライン対局。
      </p>
      <div className="home-actions">
        <Link to="/login">
          <button>ログイン</button>
        </Link>
        <Link to="/register">
          <button className="secondary">新規登録</button>
        </Link>
      </div>
    </div>
  );
}