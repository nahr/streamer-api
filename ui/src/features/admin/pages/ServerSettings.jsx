import { Box, Typography } from '@mui/material'

export function ServerSettings() {
  return (
    <Box sx={{ p: 2 }}>
      <Typography variant="h4" component="h1" gutterBottom>
        Server Settings
      </Typography>
      <Typography color="text.secondary">
        Configure server settings here.
      </Typography>
    </Box>
  )
}
