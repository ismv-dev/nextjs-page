#!/usr/bin/env bash

if [ -z "$CRON_SECRET" ]; then
  CRON_SECRET=$(node -e "console.log(require('crypto').randomBytes(32).toString('hex'))")
  echo "Generado CRON_SECRET: $CRON_SECRET"
fi