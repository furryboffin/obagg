#!/bin/bash
/app/obagg --no-syslog grpc &
sleep 3
/app/obagg --no-syslog client

