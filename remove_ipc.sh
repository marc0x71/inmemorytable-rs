ipcs -s|grep "^[0-9]x"|awk '{ system("ipcrm -s "$2)}'; rm -f /dev/shm/table_*
