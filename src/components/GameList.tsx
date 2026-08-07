import { Link } from "react-router-dom";
import type { GameSummary } from "../types";

interface GameListProps {
  games: GameSummary[];
  currentUserId: string;
  onJoin: (gameId: string) => void;
  joiningId: string | null;
}

export default function GameList({ games, currentUserId, onJoin, joiningId }: GameListProps) {
  if (games.length === 0) {
    return <p className="game-list-empty">参加待ちの対局はありません</p>;
  }

  return (
    <ul className="game-list">
      {games.map((game) => {
        const isOwnGame = game.white_user_id === currentUserId;
        return (
          <li key={game.id} className="game-list-item">
            <span>対局 #{game.id.slice(0, 8)}</span>
            {isOwnGame ? (
              <Link to={`/games/${game.id}`}>自分の対局を開く</Link>
            ) : (
              <button
                onClick={() => onJoin(game.id)}
                disabled={joiningId === game.id}
              >
                {joiningId === game.id ? "参加中..." : "参加する"}
              </button>
            )}
          </li>
        );
      })}
    </ul>
  );
}