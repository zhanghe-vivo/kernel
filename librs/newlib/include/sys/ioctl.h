#ifndef _SYS_IOCTL_H
#define _SYS_IOCTL_H

#if defined(__linux__)
#define TCGETS 21505
#endif

#if defined(__blueos__)
#define TCGETS 21505
#endif

#if defined(__linux__)
#define TCSETS 21506
#endif

#if defined(__blueos__)
#define TCSETS 21506
#endif

#if defined(__linux__)
#define TCSETSW 21507
#endif

#if defined(__blueos__)
#define TCSETSW 21507
#endif

#if defined(__linux__)
#define TCSETSF 21508
#endif

#if defined(__blueos__)
#define TCSETSF 21508
#endif

#if defined(__linux__)
#define TCGETA 21509
#endif

#if defined(__linux__)
#define TCSETA 21510
#endif

#if defined(__linux__)
#define TCSETAW 21511
#endif

#if defined(__linux__)
#define TCSETAF 21512
#endif

#if defined(__linux__)
#define TCSBRK 21513
#endif

#if defined(__blueos__)
#define TCSBRK 21513
#endif

#if defined(__linux__)
#define TCXONC 21514
#endif

#if defined(__blueos__)
#define TCXONC 21514
#endif

#if defined(__linux__)
#define TCFLSH 21515
#endif

#if defined(__blueos__)
#define TCFLSH 21515
#endif

#if defined(__linux__)
#define TIOCEXCL 21516
#endif

#if defined(__linux__)
#define TIOCNXCL 21517
#endif

#if defined(__linux__)
#define TIOCSCTTY 21518
#endif

#if defined(__blueos__)
#define TIOCSCTTY 21518
#endif

#if defined(__linux__)
#define TIOCGPGRP 21519
#endif

#if defined(__blueos__)
#define TIOCGPGRP 21519
#endif

#if defined(__linux__)
#define TIOCSPGRP 21520
#endif

#if defined(__blueos__)
#define TIOCSPGRP 21520
#endif

#if defined(__linux__)
#define TIOCOUTQ 21521
#endif

#if defined(__linux__)
#define TIOCSTI 21522
#endif

#if defined(__linux__)
#define TIOCGWINSZ 21523
#endif

#if defined(__blueos__)
#define TIOCGWINSZ 21523
#endif

#if defined(__linux__)
#define TIOCSWINSZ 21524
#endif

#if defined(__blueos__)
#define TIOCSWINSZ 21524
#endif

#if defined(__linux__)
#define TIOCMGET 21525
#endif

#if defined(__linux__)
#define TIOCMBIS 21526
#endif

#if defined(__linux__)
#define TIOCMBIC 21527
#endif

#if defined(__linux__)
#define TIOCMSET 21528
#endif

#if defined(__linux__)
#define TIOCGSOFTCAR 21529
#endif

#if defined(__linux__)
#define TIOCSSOFTCAR 21530
#endif

#if defined(__linux__)
#define FIONREAD 21531
#endif

#if defined(__blueos__)
#define FIONREAD 21531
#endif

#if defined(__linux__)
#define TIOCINQ FIONREAD
#endif

#if defined(__linux__)
#define TIOCLINUX 21532
#endif

#if defined(__linux__)
#define TIOCCONS 21533
#endif

#if defined(__linux__)
#define TIOCGSERIAL 21534
#endif

#if defined(__linux__)
#define TIOCSSERIAL 21535
#endif

#if defined(__linux__)
#define TIOCPKT 21536
#endif

#if defined(__linux__)
#define FIONBIO 21537
#endif

#if defined(__blueos__)
#define FIONBIO 21537
#endif

#if defined(__linux__)
#define TIOCNOTTY 21538
#endif

#if defined(__linux__)
#define TIOCSETD 21539
#endif

#if defined(__linux__)
#define TIOCGETD 21540
#endif

#if defined(__linux__)
#define TCSBRKP 21541
#endif

#if defined(__linux__)
#define TIOCSBRK 21543
#endif

#if defined(__linux__)
#define TIOCCBRK 21544
#endif

#if defined(__linux__)
#define TIOCGSID 21545
#endif

#if defined(__linux__)
#define TIOCGRS485 21550
#endif

#if defined(__linux__)
#define TIOCSRS485 21551
#endif

#if defined(__linux__)
#define TIOCGPTN 2147767344
#endif

#if defined(__linux__)
#define TIOCSPTLCK 1074025521
#endif

#if defined(__blueos__)
#define TIOCSPTLCK 1074025521
#endif

#if defined(__linux__)
#define TIOCGDEV 2147767346
#endif

#if defined(__linux__)
#define TCGETX 21554
#endif

#if defined(__linux__)
#define TCSETX 21555
#endif

#if defined(__linux__)
#define TCSETXF 21556
#endif

#if defined(__linux__)
#define TCSETXW 21557
#endif

#if defined(__linux__)
#define TIOCSIG 1074025526
#endif

#if defined(__linux__)
#define TIOCVHANGUP 21559
#endif

#if defined(__linux__)
#define TIOCGPKT 2147767352
#endif

#if defined(__linux__)
#define TIOCGPTLCK 2147767353
#endif

#if defined(__blueos__)
#define TIOCGPTLCK 2147767353
#endif

#if defined(__linux__)
#define TIOCGEXCL 2147767360
#endif

#if defined(__linux__)
#define TIOCGPTPEER 21569
#endif

#if defined(__linux__)
#define FIONCLEX 21584
#endif

#if defined(__linux__)
#define FIOCLEX 21585
#endif

#if defined(__linux__)
#define FIOASYNC 21586
#endif

#if defined(__linux__)
#define TIOCSERCONFIG 21587
#endif

#if defined(__linux__)
#define TIOCSERGWILD 21588
#endif

#if defined(__linux__)
#define TIOCSERSWILD 21589
#endif

#if defined(__linux__)
#define TIOCGLCKTRMIOS 21590
#endif

#if defined(__linux__)
#define TIOCSLCKTRMIOS 21591
#endif

#if defined(__linux__)
#define TIOCSERGSTRUCT 21592
#endif

#if defined(__linux__)
#define TIOCSERGETLSR 21593
#endif

#if defined(__linux__)
#define TIOCSERGETMULTI 21594
#endif

#if defined(__linux__)
#define TIOCSERSETMULTI 21595
#endif

#if defined(__linux__)
#define TIOCMIWAIT 21596
#endif

#if defined(__linux__)
#define TIOCGICOUNT 21597
#endif

#if defined(__linux__)
#define FIOQSIZE 21600
#endif

#if defined(__linux__)
#define TIOCPKT_DATA 0
#endif

#if defined(__linux__)
#define TIOCPKT_FLUSHREAD 1
#endif

#if defined(__linux__)
#define TIOCPKT_FLUSHWRITE 2
#endif

#if defined(__linux__)
#define TIOCPKT_STOP 4
#endif

#if defined(__linux__)
#define TIOCPKT_START 8
#endif

#if defined(__linux__)
#define TIOCPKT_NOSTOP 16
#endif

#if defined(__linux__)
#define TIOCPKT_DOSTOP 32
#endif

#if defined(__linux__)
#define TIOCPKT_IOCTL 64
#endif

#if defined(__linux__)
#define TIOCSER_TEMT 1
#endif

#if defined(__linux__)
#define TIOCM_LE 1
#endif

#if defined(__linux__)
#define TIOCM_DTR 2
#endif

#if defined(__linux__)
#define TIOCM_RTS 4
#endif

#if defined(__linux__)
#define TIOCM_ST 8
#endif

#if defined(__linux__)
#define TIOCM_SR 16
#endif

#if defined(__linux__)
#define TIOCM_CTS 32
#endif

#if defined(__linux__)
#define TIOCM_CAR 64
#endif

#if defined(__linux__)
#define TIOCM_RNG 128
#endif

#if defined(__linux__)
#define TIOCM_DSR 256
#endif

#if defined(__linux__)
#define TIOCM_CD TIOCM_CAR
#endif

#if defined(__linux__)
#define TIOCM_RI TIOCM_RNG
#endif

#if defined(__linux__)
#define TIOCM_OUT1 8192
#endif

#if defined(__linux__)
#define TIOCM_OUT2 16384
#endif

#if defined(__linux__)
#define TIOCM_LOOP 32768
#endif

#if defined(__linux__)
#define N_TTY 0
#endif

#if defined(__linux__)
#define N_SLIP 1
#endif

#if defined(__linux__)
#define N_MOUSE 2
#endif

#if defined(__linux__)
#define N_PPP 3
#endif

#if defined(__linux__)
#define N_STRIP 4
#endif

#if defined(__linux__)
#define N_AX25 5
#endif

#if defined(__linux__)
#define N_X25 6
#endif

#if defined(__linux__)
#define N_6PACK 7
#endif

#if defined(__linux__)
#define N_MASC 8
#endif

#if defined(__linux__)
#define N_R3964 9
#endif

#if defined(__linux__)
#define N_PROFIBUS_FDL 10
#endif

#if defined(__linux__)
#define N_IRDA 11
#endif

#if defined(__linux__)
#define N_SMSBLOCK 12
#endif

#if defined(__linux__)
#define N_HDLC 13
#endif

#if defined(__linux__)
#define N_SYNC_PPP 14
#endif

#if defined(__linux__)
#define N_HCI 15
#endif

#if defined(__linux__)
#define FIOSETOWN 35073
#endif

#if defined(__linux__)
#define SIOCSPGRP 35074
#endif

#if defined(__linux__)
#define FIOGETOWN 35075
#endif

#if defined(__linux__)
#define SIOCGPGRP 35076
#endif

#if defined(__linux__)
#define SIOCATMARK 35077
#endif

#if defined(__blueos__)
#define SIOCATMARK 35077
#endif

#if defined(__linux__)
#define SIOCGSTAMP 35078
#endif

#if defined(__linux__)
#define SIOCGSTAMPNS 35079
#endif

#if defined(__linux__)
#define SIOCADDRT 35083
#endif

#if defined(__linux__)
#define SIOCDELRT 35084
#endif

#if defined(__linux__)
#define SIOCRTMSG 35085
#endif

#if defined(__linux__)
#define SIOCGIFNAME 35088
#endif

#if defined(__linux__)
#define SIOCSIFLINK 35089
#endif

#if defined(__linux__)
#define SIOCGIFCONF 35090
#endif

#if defined(__linux__)
#define SIOCGIFFLAGS 35091
#endif

#if defined(__linux__)
#define SIOCSIFFLAGS 35092
#endif

#if defined(__linux__)
#define SIOCGIFADDR 35093
#endif

#if defined(__linux__)
#define SIOCSIFADDR 35094
#endif

#if defined(__linux__)
#define SIOCGIFDSTADDR 35095
#endif

#if defined(__linux__)
#define SIOCSIFDSTADDR 35096
#endif

#if defined(__linux__)
#define SIOCGIFBRDADDR 35097
#endif

#if defined(__linux__)
#define SIOCSIFBRDADDR 35098
#endif

#if defined(__linux__)
#define SIOCGIFNETMASK 35099
#endif

#if defined(__linux__)
#define SIOCSIFNETMASK 35100
#endif

#if defined(__linux__)
#define SIOCGIFMETRIC 35101
#endif

#if defined(__linux__)
#define SIOCSIFMETRIC 35102
#endif

#if defined(__linux__)
#define SIOCGIFMEM 35103
#endif

#if defined(__linux__)
#define SIOCSIFMEM 35104
#endif

#if defined(__linux__)
#define SIOCGIFMTU 35105
#endif

#if defined(__linux__)
#define SIOCSIFMTU 35106
#endif

#if defined(__linux__)
#define SIOCSIFNAME 35107
#endif

#if defined(__linux__)
#define SIOCSIFHWADDR 35108
#endif

#if defined(__linux__)
#define SIOCGIFENCAP 35109
#endif

#if defined(__linux__)
#define SIOCSIFENCAP 35110
#endif

#if defined(__linux__)
#define SIOCGIFHWADDR 35111
#endif

#if defined(__linux__)
#define SIOCGIFSLAVE 35113
#endif

#if defined(__linux__)
#define SIOCSIFSLAVE 35120
#endif

#if defined(__linux__)
#define SIOCADDMULTI 35121
#endif

#if defined(__linux__)
#define SIOCDELMULTI 35122
#endif

#if defined(__linux__)
#define SIOCGIFINDEX 35123
#endif

#if defined(__linux__)
#define SIOGIFINDEX SIOCGIFINDEX
#endif

#if defined(__linux__)
#define SIOCSIFPFLAGS 35124
#endif

#if defined(__linux__)
#define SIOCGIFPFLAGS 35125
#endif

#if defined(__linux__)
#define SIOCDIFADDR 35126
#endif

#if defined(__linux__)
#define SIOCSIFHWBROADCAST 35127
#endif

#if defined(__linux__)
#define SIOCGIFCOUNT 35128
#endif

#if defined(__linux__)
#define SIOCGIFBR 35136
#endif

#if defined(__linux__)
#define SIOCSIFBR 35137
#endif

#if defined(__linux__)
#define SIOCGIFTXQLEN 35138
#endif

#if defined(__linux__)
#define SIOCSIFTXQLEN 35139
#endif

#if defined(__linux__)
#define SIOCDARP 35155
#endif

#if defined(__linux__)
#define SIOCGARP 35156
#endif

#if defined(__linux__)
#define SIOCSARP 35157
#endif

#if defined(__linux__)
#define SIOCDRARP 35168
#endif

#if defined(__linux__)
#define SIOCGRARP 35169
#endif

#if defined(__linux__)
#define SIOCSRARP 35170
#endif

#if defined(__linux__)
#define SIOCGIFMAP 35184
#endif

#if defined(__linux__)
#define SIOCSIFMAP 35185
#endif

#if defined(__linux__)
#define SIOCADDDLCI 35200
#endif

#if defined(__linux__)
#define SIOCDELDLCI 35201
#endif

#if defined(__linux__)
#define SIOCDEVPRIVATE 35312
#endif

#if defined(__linux__)
#define SIOCPROTOPRIVATE 35296
#endif

struct sgttyb {
  char sg_ispeed;
  char sg_ospeed;
  char sg_erase;
  char sg_kill;
  unsigned short sg_flags;
};

struct winsize {
  unsigned short ws_row;
  unsigned short ws_col;
  unsigned short ws_xpixel;
  unsigned short ws_ypixel;
};

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

#if defined(__linux__)
int ioctl(int fd, unsigned long request, void *out);
#endif

#if defined(__blueos__)
int ioctl(int fd, unsigned long request, void *out);
#endif

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _SYS_IOCTL_H */
