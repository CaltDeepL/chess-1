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
    <div className="game-tile-grid">
      {games.map((game) => {
        const isOwnGame = game.white_user_id === currentUserId;
        return (
          <div key={game.id} className="game-tile">
            <div className="game-tile-id">#{game.id.slice(0, 8)}</div>
            <div className="game-tile-meta">
              <span className="game-tile-dot" />
              対局相手を待っています
            </div>
            {isOwnGame ? (
              <Link to={`/games/${game.id}`} className="game-tile-action">
                自分の対局を開く
              </Link>
            ) : (
              <button
                className="game-tile-action"
                onClick={() => onJoin(game.id)}
                disabled={joiningId === game.id}
              >
                {joiningId === game.id ? "参加中..." : "参加する"}
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}