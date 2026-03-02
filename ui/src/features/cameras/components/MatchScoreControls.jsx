import {
  Box,
  Typography,
  Button,
  IconButton,
  Stack,
} from '@mui/material'
import AddIcon from '@mui/icons-material/Add'
import RemoveIcon from '@mui/icons-material/Remove'
import StopIcon from '@mui/icons-material/Stop'
import { formatMatchWinner } from '../../../utils/format.js'

/**
 * Score controls for a pool match: +/- buttons, race-to display, end match.
 * @param {Object} props
 * @param {Object} props.match - Match with player_one, player_two, end_time, match_type?
 * @param {boolean} props.scoreUpdating
 * @param {(player: 1|2, delta: number) => void} props.onScoreChange
 * @param {() => void} props.onEndMatch
 * @param {boolean} [props.showEndedMessage] - Show "X won" / "Ended early" when match ended
 */
export function MatchScoreControls({
  match,
  scoreUpdating,
  onScoreChange,
  onEndMatch,
  showEndedMessage = true,
}) {
  const isActive = !match.end_time
  const isPractice = match.match_type === 'practice'
  const p1 = match.player_one
  const p1Maxed = p1.race_to > 0 && p1.games_won >= p1.race_to

  return (
    <Stack spacing={2}>
      <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2} alignItems="center" flexWrap="wrap">
        <Box display="flex" alignItems="center" gap={0.5}>
          <IconButton
            size="small"
            onClick={() => onScoreChange(1, -1)}
            disabled={scoreUpdating || isPractice || p1.games_won === 0}
            aria-label={isPractice ? 'Decrease racks' : 'Decrease player 1 score'}
          >
            <RemoveIcon />
          </IconButton>
          <Typography variant="h5" component="span" sx={{ minWidth: 40, textAlign: 'center' }}>
            {p1.games_won}
          </Typography>
          <IconButton
            size="small"
            onClick={() => onScoreChange(1, 1)}
            disabled={scoreUpdating || p1Maxed}
            aria-label={isPractice ? 'Increase racks' : 'Increase player 1 score'}
          >
            <AddIcon />
          </IconButton>
          <Typography sx={{ ml: 1 }}>{p1.name}</Typography>
          <Typography color="text.secondary" variant="body2" sx={{ ml: 0.5 }}>
            {isPractice
              ? (p1.race_to > 0 ? `racks (target ${p1.race_to})` : 'racks')
              : `(race to ${p1.race_to})`}
          </Typography>
        </Box>
        {!isPractice && (
          <>
            <Typography color="text.secondary">vs</Typography>
            <Box display="flex" alignItems="center" gap={0.5}>
              <IconButton
                size="small"
                onClick={() => onScoreChange(2, -1)}
                disabled={scoreUpdating || match.player_two.games_won === 0}
                aria-label="Decrease player 2 score"
              >
                <RemoveIcon />
              </IconButton>
              <Typography variant="h5" component="span" sx={{ minWidth: 40, textAlign: 'center' }}>
                {match.player_two.games_won}
              </Typography>
              <IconButton
                size="small"
                onClick={() => onScoreChange(2, 1)}
                disabled={scoreUpdating || match.player_two.games_won >= match.player_two.race_to}
                aria-label="Increase player 2 score"
              >
                <AddIcon />
              </IconButton>
              <Typography sx={{ ml: 1 }}>{match.player_two.name}</Typography>
              <Typography color="text.secondary" variant="body2" sx={{ ml: 0.5 }}>
                (race to {match.player_two.race_to})
              </Typography>
            </Box>
          </>
        )}
      </Stack>
      {isActive && (
        <Button
          startIcon={<StopIcon />}
          variant="outlined"
          color="secondary"
          onClick={onEndMatch}
          disabled={scoreUpdating}
        >
          {isPractice ? 'End practice' : 'End match early'}
        </Button>
      )}
      {!isActive && showEndedMessage && (
        <Typography color="text.secondary" variant="body2">
          {formatMatchWinner(match)}
        </Typography>
      )}
    </Stack>
  )
}
